//! 分层自适应推荐(§4.4):熵最大化 / 精确前瞻。

use std::collections::HashMap;

use crate::core::model::Record;
use crate::core::model::Settings;
use crate::core::solver::enumerate_space;
use crate::core::solver::filter_candidates;

pub const FULL_SPACE_LIMIT: usize = 20_000;
pub const SAMPLE_SIZE: usize = 2048;

pub(crate) fn guess_space(settings: &Settings) -> Vec<Vec<u8>> {
    let space = enumerate_space(settings);
    if space.len() <= FULL_SPACE_LIMIT {
        space
    } else {
        sample_guesses(&space, SAMPLE_SIZE)
    }
}

pub(crate) fn sample_guesses(space: &[Vec<u8>], n: usize) -> Vec<Vec<u8>> {
    if n >= space.len() {
        return space.to_vec();
    }
    let mut rng: u64 = 0x9E37_79B9_7F4A_7C15; // 固定种子,结果可复现(§4.4)
    let mut used = vec![false; space.len()];
    let mut chosen: Vec<usize> = Vec::with_capacity(n);
    while chosen.len() < n {
        // xorshift64*
        rng ^= rng >> 12;
        rng ^= rng << 25;
        rng ^= rng >> 27;
        let idx = (rng.wrapping_mul(0x2545_F491_4F6C_DD1D) as usize) % space.len();
        if !used[idx] {
            used[idx] = true;
            chosen.push(idx);
        }
    }
    chosen.sort_unstable(); // 保持枚举序,保证平手裁决确定
    chosen.into_iter().map(|i| space[i].clone()).collect()
}

pub(crate) fn entropy_best(candidates: &[Vec<u8>], guesses: &[Vec<u8>]) -> (Vec<u8>, f64, usize) {
    let n = candidates.len() as f64;
    let cand_set: std::collections::HashSet<&Vec<u8>> = candidates.iter().collect();
    let mut best: Option<(Vec<u8>, f64, usize, bool)> = None; // (guess, 熵, 最坏桶, 是否属候选集)
    for g in guesses {
        let mut buckets: HashMap<(u8, u8), usize> = HashMap::new();
        for c in candidates {
            *buckets.entry(crate::core::judge::judge(g, c)).or_insert(0) += 1;
        }
        // 对桶大小排序后求和,消除 HashMap 遍历序带来的浮点误差
        let mut sizes: Vec<usize> = buckets.values().copied().collect();
        sizes.sort_unstable();
        let h = -sizes
            .iter()
            .map(|&v| { let p = v as f64 / n; p * p.log2() })
            .sum::<f64>();
        let worst = sizes[sizes.len() - 1];
        let in_cand = cand_set.contains(g);
        let replace = match &best {
            None => true,
            Some((_, bh, _, b_in)) => {
                h > bh + 1e-9 || ((h - bh).abs() <= 1e-9 && in_cand && !*b_in)
            }
        };
        if replace {
            best = Some((g.clone(), h, worst, in_cand));
        }
    }
    let (g, h, w, _) = best.expect("guesses 非空");
    (g, h, w)
}

pub const LOOKAHEAD_CANDIDATE_LIMIT: usize = 30;

#[derive(Debug, Clone, PartialEq)]
pub enum Bound {
    /// 精确前瞻:最多还需 N 步(含本次猜测)
    GuaranteedSteps(usize),
    /// 熵推荐:期望参考,非保证(期望信息量 bits / 最坏桶大小)
    Expected { entropy_bits: f64, worst_bucket: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Recommendation {
    Answer(Vec<u8>),                          // 剩余候选唯一
    Guess { guess: Vec<u8>, bound: Bound },   // 推荐猜测 + 最坏情况说明
}

/// 公开入口(§4.4):过滤出候选后分层推荐。
pub fn recommend(settings: &Settings, records: &[Record]) -> Recommendation {
    let candidates = filter_candidates(settings, records);
    assert!(
        !candidates.is_empty(),
        "recommend: 候选为空属于矛盾,调用方(solve/GUI)应先检查"
    );
    recommend_for(settings, &candidates)
}

/// 由已知候选集直接推荐(GUI 每帧缓存后调用;要求候选非空)。
pub(crate) fn recommend_for(settings: &Settings, candidates: &[Vec<u8>]) -> Recommendation {
    match candidates.len() {
        0 => panic!("recommend_for: 候选为空属于矛盾,调用方(solve/GUI)应先检查"),
        1 => Recommendation::Answer(candidates[0].clone()),
        2 => Recommendation::Guess {
            // 猜其中之一:命中即结束,未中则另一个即答案,必 ≤2 步(§4.4)
            guess: candidates[0].clone(),
            bound: Bound::GuaranteedSteps(2),
        },
        n if n <= LOOKAHEAD_CANDIDATE_LIMIT => {
            let guesses = guess_space(settings);
            if let Some((guess, steps)) = lookahead_best(candidates, &guesses, 3) {
                Recommendation::Guess { guess, bound: Bound::GuaranteedSteps(steps) }
            } else {
                // 深度上限内无法给出保证(理论下不会发生):回退熵推荐并如实标注为期望值
                let (guess, h, worst) = entropy_pick(settings, candidates);
                Recommendation::Guess {
                    guess,
                    bound: Bound::Expected { entropy_bits: h, worst_bucket: worst },
                }
            }
        }
        _ => {
            let (guess, h, worst) = entropy_pick(settings, candidates);
            Recommendation::Guess {
                guess,
                bound: Bound::Expected { entropy_bits: h, worst_bucket: worst },
            }
        }
    }
}

/// 熵层打分:极端配置下候选侧同样固定种子采样,保证同步计算秒级内(§4.4)。
/// 仅用于打分;Bound::Expected 本就标注为期望参考而非保证。
fn entropy_pick(settings: &Settings, candidates: &[Vec<u8>]) -> (Vec<u8>, f64, usize) {
    let guesses = guess_space(settings);
    if candidates.len() > FULL_SPACE_LIMIT {
        let sampled = sample_guesses(candidates, SAMPLE_SIZE);
        entropy_best(&sampled, &guesses)
    } else {
        entropy_best(candidates, &guesses)
    }
}

/// 记忆化键:(剩余深度, 候选集扁平化)。值 None = 该预算下无法证明。
fn lookahead_steps(
    cands: &[Vec<u8>],
    guesses: &[Vec<u8>],
    terminal: (u8, u8),
    budget: usize,
    memo: &mut HashMap<(usize, Vec<u8>), Option<usize>>,
) -> Option<usize> {
    if cands.len() == 1 {
        return Some(1); // 已知答案,提交 1 次
    }
    if budget == 0 {
        return None;
    }
    let key = (budget, cands.iter().flat_map(|c| c.iter().copied()).collect::<Vec<u8>>());
    if let Some(cached) = memo.get(&key) {
        return *cached;
    }
    let mut best: Option<usize> = None;
    for g in guesses {
        let mut buckets: HashMap<(u8, u8), Vec<Vec<u8>>> = HashMap::new();
        for c in cands {
            buckets.entry(crate::core::judge::judge(g, c)).or_default().push(c.clone());
        }
        let mut worst = 0usize;
        let mut provable = true;
        for (fb, bucket) in &buckets {
            if *fb == terminal {
                continue; // 猜中答案,终局
            }
            match lookahead_steps(bucket, guesses, terminal, budget - 1, memo) {
                Some(v) => worst = worst.max(v),
                None => {
                    provable = false;
                    break;
                }
            }
        }
        if provable {
            let total = 1 + worst;
            if best.is_none_or(|b| total < b) {
                best = Some(total);
            }
        }
    }
    memo.insert(key, best);
    best
}

/// minimax 前瞻:返回保证最少剩余步数(含本次猜测)的猜测与该步数;
/// 深度预算用尽无法证明 → None(理论下 ≤30 候选、cap=3 不会发生)。
/// 步数语义:steps(C)=1+min_g max_{非终局桶} steps(桶);反馈=(slots,0) 为终局桶;
/// 单候选桶 steps=1(直接提交)。
pub(crate) fn lookahead_best(
    candidates: &[Vec<u8>],
    guesses: &[Vec<u8>],
    depth_cap: usize,
) -> Option<(Vec<u8>, usize)> {
    debug_assert!(!candidates.is_empty() && !guesses.is_empty());
    let slots = candidates[0].len() as u8;
    let terminal = (slots, 0);
    let mut memo: HashMap<(usize, Vec<u8>), Option<usize>> = HashMap::new();
    let best = lookahead_steps(candidates, guesses, terminal, depth_cap, &mut memo)?;
    // 找到达成该保证的第一个猜测(枚举序,裁决确定)
    for g in guesses {
        let mut buckets: HashMap<(u8, u8), Vec<Vec<u8>>> = HashMap::new();
        for c in candidates {
            buckets.entry(crate::core::judge::judge(g, c)).or_default().push(c.clone());
        }
        let mut worst = 0usize;
        let mut provable = true;
        for (fb, bucket) in &buckets {
            if *fb == terminal {
                continue;
            }
            match lookahead_steps(bucket, guesses, terminal, depth_cap - 1, &mut memo) {
                Some(v) => worst = worst.max(v),
                None => {
                    provable = false;
                    break;
                }
            }
        }
        if provable && 1 + worst == best {
            return Some((g.clone(), best));
        }
    }
    unreachable!("已由 lookahead_steps 证明存在达成保证的猜测")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_space() -> Vec<Vec<u8>> {
        enumerate_space(&Settings::default())
    }

    fn ab_candidates() -> Vec<Vec<u8>> {
        filter_candidates(
            &Settings::default(),
            &[
                Record::new(vec![3, 1, 2, 0], 1, 0),
                Record::new(vec![3, 1, 2, 5], 1, 0),
            ],
        )
    }

    #[test]
    fn entropy_first_move_on_6x4_is_0123() {
        // 穷举脚本已验证:全 1296 空间上熵最大者(先枚举序取首个 argmax)
        let space = default_space();
        let (g, h, worst) = entropy_best(&space, &space);
        assert_eq!(g, vec![0, 1, 2, 3]); // 红蓝紫橙
        assert!((h - 3.056_671).abs() < 1e-4);
        assert_eq!(worst, 312);
    }

    #[test]
    fn entropy_after_two_real_records() {
        // 24 候选场景(脚本验证):最佳猜测 [3,4,4,1](橙黄黄蓝,不在候选集内)
        let cands = ab_candidates();
        let space = default_space();
        let (g, h, worst) = entropy_best(&cands, &space);
        assert_eq!(g, vec![3, 4, 4, 1]);
        assert!((h - 3.173_533).abs() < 1e-4);
        assert_eq!(worst, 5);
    }

    #[test]
    fn entropy_is_deterministic() {
        let cands = ab_candidates();
        let space = default_space();
        assert_eq!(entropy_best(&cands, &space), entropy_best(&cands, &space));
    }

    #[test]
    fn sample_covers_small_space_entirely() {
        // n ≥ 空间大小时返回全量(采样路径与全量路径一致的回归基础,规格 §6)
        let space = enumerate_space(&Settings { colors: 4, slots: 4, repeats: true });
        assert_eq!(space.len(), 256);
        assert_eq!(sample_guesses(&space, SAMPLE_SIZE), space);
    }

    #[test]
    fn sample_is_deterministic_and_distinct() {
        let space = enumerate_space(&Settings { colors: 8, slots: 6, repeats: true });
        let a = sample_guesses(&space, SAMPLE_SIZE);
        let b = sample_guesses(&space, SAMPLE_SIZE);
        assert_eq!(a, b); // 固定种子 → 结果可复现
        assert_eq!(a.len(), SAMPLE_SIZE);
        let uniq: std::collections::HashSet<_> = a.iter().collect();
        assert_eq!(uniq.len(), SAMPLE_SIZE); // 互不相同
    }

    #[test]
    fn guess_space_full_below_limit_and_sampled_above() {
        assert_eq!(guess_space(&Settings::default()).len(), 1296);           // 6^4 全量
        assert_eq!(guess_space(&Settings { colors: 8, slots: 6, repeats: true }).len(), SAMPLE_SIZE); // 8^6 采样
    }

    fn c3() -> Vec<Vec<u8>> {
        vec![vec![0, 0, 0, 0], vec![0, 0, 0, 1], vec![1, 1, 1, 1]]
    }

    #[test]
    fn lookahead_c3_guarantees_2() {
        // 脚本验证:argmin(先枚举序)= [0,0,0,0],最优保证 2 步(3 候选不可能 1 步)
        let guesses = default_space();
        assert_eq!(lookahead_best(&c3(), &guesses, 3), Some((vec![0, 0, 0, 0], 2)));
    }

    #[test]
    fn lookahead_depth0_falls_back_to_none() {
        let guesses = default_space();
        assert_eq!(lookahead_best(&c3(), &guesses, 0), None);
    }

    #[test]
    fn lookahead_24_candidates_guarantees_3() {
        // 脚本验证:两条真实记录后 24 候选,最优保证 3 步,猜测 [0,0,4,4](红红黄黄)
        let guesses = default_space();
        assert_eq!(lookahead_best(&ab_candidates(), &guesses, 3), Some((vec![0, 0, 4, 4], 3)));
    }

    #[test]
    fn recommend_first_move_is_entropy() {
        // 1296 候选 > 30 → 熵层(熵值用近似比较,f64 精确相等不可靠)
        let rec = recommend(&Settings::default(), &[]);
        match rec {
            Recommendation::Guess {
                guess,
                bound: Bound::Expected { entropy_bits, worst_bucket },
            } => {
                assert_eq!(guess, vec![0, 1, 2, 3]);
                assert!((entropy_bits - 3.056_671).abs() < 1e-4);
                assert_eq!(worst_bucket, 312);
            }
            other => panic!("应熵推荐,实际 {other:?}"),
        }
    }

    #[test]
    fn recommend_two_candidates_special_case() {
        // 脚本验证:这 5 条记录后恰好剩 {紫紫紫黄, 黄紫紫紫} 两个候选
        let records = vec![
            Record::new(vec![3, 1, 2, 0], 1, 0),
            Record::new(vec![3, 1, 2, 5], 1, 0),
            Record::new(vec![0, 1, 2, 3], 1, 0),
            Record::new(vec![0, 0, 1, 1], 0, 0),
            Record::new(vec![2, 4, 2, 2], 2, 2),
        ];
        let rec = recommend(&Settings::default(), &records);
        assert_eq!(
            rec,
            Recommendation::Guess { guess: vec![2, 2, 2, 4], bound: Bound::GuaranteedSteps(2) }
        );
    }

    #[test]
    fn recommend_24_candidates_uses_lookahead() {
        let records = vec![
            Record::new(vec![3, 1, 2, 0], 1, 0),
            Record::new(vec![3, 1, 2, 5], 1, 0),
        ];
        let rec = recommend(&Settings::default(), &records);
        assert_eq!(
            rec,
            Recommendation::Guess { guess: vec![0, 0, 4, 4], bound: Bound::GuaranteedSteps(3) }
        );
    }

    #[test]
    fn recommend_dispatch_boundary() {
        // 31 候选 → 熵层;唯一候选 → Answer
        let space = default_space();
        let s = Settings::default();
        let c31 = &space[..31];
        assert!(matches!(
            recommend_for(&s, c31),
            Recommendation::Guess { bound: Bound::Expected { .. }, .. }
        ));
        assert_eq!(
            recommend_for(&s, &space[..1]),
            Recommendation::Answer(vec![0, 0, 0, 0])
        );
    }

    #[test]
    fn guarantee_simulated_to_end() {
        // 规格 §6:模拟到终局核对保证步数确实成立。
        // worst_case_steps:对每个可能真答案,沿推荐猜测递归模拟,返回最大提交次数。
        fn worst_case_steps(settings: &Settings, records: &[Record]) -> usize {
            let cands = filter_candidates(settings, records);
            if cands.len() <= 1 {
                return 1;
            }
            let guess = match recommend_for(settings, &cands) {
                Recommendation::Answer(_) => return 1,
                Recommendation::Guess { guess, .. } => guess,
            };
            let mut worst = 0usize;
            for ans in &cands {
                let fb = crate::core::judge::judge(&guess, ans);
                if fb == (settings.slots as u8, 0) {
                    worst = worst.max(1);
                    continue;
                }
                let mut next = records.to_vec();
                next.push(Record { guess: guess.clone(), exact: fb.0, partial: fb.1, enabled: true });
                worst = worst.max(1 + worst_case_steps(settings, &next));
            }
            worst
        }
        let s = Settings::default();
        let ab = vec![
            Record::new(vec![3, 1, 2, 0], 1, 0),
            Record::new(vec![3, 1, 2, 5], 1, 0),
        ];
        assert_eq!(worst_case_steps(&s, &ab), 3); // 与 GuaranteedSteps(3) 一致
        let two = vec![
            Record::new(vec![3, 1, 2, 0], 1, 0),
            Record::new(vec![3, 1, 2, 5], 1, 0),
            Record::new(vec![0, 1, 2, 3], 1, 0),
            Record::new(vec![0, 0, 1, 1], 0, 0),
            Record::new(vec![2, 4, 2, 2], 2, 2),
        ];
        assert_eq!(worst_case_steps(&s, &two), 2); // 与 GuaranteedSteps(2) 一致
    }

    #[test]
    fn entropy_samples_candidate_side_on_huge_spaces() {
        // 8⁵=32768 > FULL_SPACE_LIMIT:候选侧采样路径被触发;
        // 结果仍须是合法猜测且确定(两调用一致),耗时秒级内
        let s = Settings { colors: 8, slots: 5, repeats: true };
        let rec = recommend(&s, &[]);
        match rec {
            Recommendation::Guess { ref guess, bound: Bound::Expected { .. } } => {
                assert_eq!(guess.len(), 5);
                assert!(guess.iter().all(|&g| (g as usize) < 8));
            }
            other => panic!("应熵推荐,实际 {other:?}"),
        }
        assert_eq!(recommend(&s, &[]), rec); // 固定种子 → 可复现
    }
}
