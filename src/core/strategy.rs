//! 分层自适应推荐(§4.4):熵最大化 / 精确前瞻。

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

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

#[cfg(test)]
pub(crate) fn entropy_best(candidates: &[Vec<u8>], guesses: &[Vec<u8>]) -> (Vec<u8>, f64, usize) {
    entropy_best_cancellable(candidates, guesses, &AtomicBool::new(false)).unwrap()
}

/// 同一个完整反馈分区支持熵、最坏桶和平均剩余候选数,避免重复判定。
fn feedback_statistics(
    guess: &[u8],
    candidates: &[Vec<u8>],
    cancelled: &AtomicBool,
) -> Option<(f64, usize, f64)> {
    let mut buckets: HashMap<(u8, u8), usize> = HashMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        if index % 128 == 0 && cancelled.load(Ordering::Relaxed) { return None; }
        *buckets.entry(crate::core::judge::judge(guess, candidate)).or_default() += 1;
    }
    // 排序消除 HashMap 遍历序带来的浮点差异,保留原有平手裁决。
    let mut sizes: Vec<usize> = buckets.values().copied().collect();
    sizes.sort_unstable();
    let n = candidates.len() as f64;
    let entropy_bits = -sizes.iter().map(|&count| {
        let probability = count as f64 / n;
        probability * probability.log2()
    }).sum::<f64>();
    let expected_remaining = sizes.iter().map(|&count| {
        let count = count as f64;
        count * count / n
    }).sum::<f64>();
    Some((entropy_bits, *sizes.last().expect("候选非空"), expected_remaining))
}

fn entropy_best_cancellable(
    candidates: &[Vec<u8>],
    guesses: &[Vec<u8>],
    cancelled: &AtomicBool,
) -> Option<(Vec<u8>, f64, usize)> {
    if cancelled.load(Ordering::Relaxed) { return None; }
    let cand_set: std::collections::HashSet<&Vec<u8>> = candidates.iter().collect();
    // 各猜测的打分相互独立 → 并行计算;浮点求和仍在单猜测内按同一顺序完成,结果与串行逐位一致
    let scores: Option<Vec<(f64, usize, bool)>> = guesses
        .par_iter()
        .map(|g| {
            if cancelled.load(Ordering::Relaxed) { return None; }
            let (h, worst, _) = feedback_statistics(g, candidates, cancelled)?;
            Some((h, worst, cand_set.contains(g)))
        })
        .collect();
    // 按枚举序串行归约,保持原有平手裁决(先枚举序取首个 argmax,平手偏向候选集内)
    let mut best: Option<(Vec<u8>, f64, usize, bool)> = None; // (guess, 熵, 最坏桶, 是否属候选集)
    for (g, (h, worst, in_cand)) in guesses.iter().zip(scores?) {
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
    if cancelled.load(Ordering::Relaxed) { return None; }
    Some((g, h, w))
}

pub const LOOKAHEAD_CANDIDATE_LIMIT: usize = 30;

#[derive(Debug, Clone, PartialEq)]
pub enum Bound {
    /// 全候选前瞻证明:最多还需 N 步(含本次猜测和最终提交)。
    /// 搜索可能只覆盖部分合法猜测,此上界不声称全局最优。
    GuaranteedSteps(usize),
    /// 熵推荐:期望参考,非保证(期望信息量 bits / 最坏桶大小)
    Expected { entropy_bits: f64, worst_bucket: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Recommendation {
    Answer(Vec<u8>),                          // 剩余候选唯一
    Guess { guess: Vec<u8>, bound: Bound },   // 推荐猜测 + 最坏情况说明
}

/// 指标均覆盖全部当前候选;期望值假设这些候选等可能。
#[derive(Debug, Clone, PartialEq)]
pub struct RecommendationEvidence {
    pub candidate_count: usize,
    pub is_possible_answer: bool,
    pub expected_remaining: f64,
    pub worst_bucket: usize,
    pub entropy_bits: f64,
    /// 搜索猜测或评分候选使用了采样,不表示显示指标是采样估计。
    pub sampled_search: bool,
}

pub fn recommendation_evidence(
    settings: &Settings,
    candidates: &[Vec<u8>],
    recommendation: &Recommendation,
) -> RecommendationEvidence {
    assert!(!candidates.is_empty(), "推荐证据要求候选非空");
    let guess = match recommendation {
        Recommendation::Answer(answer) => answer,
        Recommendation::Guess { guess, .. } => guess,
    };
    let (entropy_bits, worst_bucket, expected_remaining) =
        feedback_statistics(guess, candidates, &AtomicBool::new(false)).unwrap();
    let possible_guesses = if settings.repeats {
        settings.colors.pow(settings.slots as u32)
    } else {
        (0..settings.slots).map(|offset| settings.colors - offset).product()
    };
    RecommendationEvidence {
        candidate_count: candidates.len(),
        is_possible_answer: candidates.contains(guess),
        expected_remaining,
        worst_bucket,
        entropy_bits,
        sampled_search: matches!(recommendation, Recommendation::Guess { .. })
            && candidates.len() > 2
            && (possible_guesses > FULL_SPACE_LIMIT || candidates.len() > FULL_SPACE_LIMIT),
    }
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
    recommend_for_cancellable(settings, candidates, &AtomicBool::new(false)).unwrap()
}

/// 先提供熵建议,不等待小候选集合的前瞻证明。
pub fn recommend_quick_for(settings: &Settings, candidates: &[Vec<u8>]) -> Recommendation {
    recommend_quick_for_cancellable(settings, candidates, &AtomicBool::new(false)).unwrap()
}

pub(crate) fn recommend_quick_for_cancellable(
    settings: &Settings,
    candidates: &[Vec<u8>],
    cancelled: &AtomicBool,
) -> Option<Recommendation> {
    if cancelled.load(Ordering::Relaxed) { return None; }
    let recommendation = match candidates.len() {
        0 => panic!("recommend_for: 候选为空属于矛盾,调用方(solve/GUI)应先检查"),
        1 => Recommendation::Answer(candidates[0].clone()),
        2 => Recommendation::Guess {
            // 猜其中之一:命中即结束,未中则另一个即答案,必 ≤2 步(§4.4)
            guess: candidates[0].clone(),
            bound: Bound::GuaranteedSteps(2),
        },
        _ => {
            let (guess, h, worst) = entropy_pick_cancellable(settings, candidates, cancelled)?;
            Recommendation::Guess {
                guess,
                bound: Bound::Expected { entropy_bits: h, worst_bucket: worst },
            }
        }
    };
    if cancelled.load(Ordering::Relaxed) { return None; }
    Some(recommendation)
}

/// 取消会使本次搜索返回 None,不会将未完成搜索当作步数保证。
pub(crate) fn recommend_for_cancellable(
    settings: &Settings,
    candidates: &[Vec<u8>],
    cancelled: &AtomicBool,
) -> Option<Recommendation> {
    if cancelled.load(Ordering::Relaxed) { return None; }
    if (3..=LOOKAHEAD_CANDIDATE_LIMIT).contains(&candidates.len()) {
        let guesses = guess_space(settings);
        let result = lookahead_best_cancellable(candidates, &guesses, 3, cancelled);
        if cancelled.load(Ordering::Relaxed) { return None; }
        if let Some((guess, steps)) = result {
            return Some(Recommendation::Guess { guess, bound: Bound::GuaranteedSteps(steps) });
        }
        // 预算内没有证明时如实回退到熵建议。
    }
    recommend_quick_for_cancellable(settings, candidates, cancelled)
}

/// 大空间采样只决定选哪个猜测;返回的指标始终在全部候选上复核。
#[cfg(test)]
fn entropy_pick(settings: &Settings, candidates: &[Vec<u8>]) -> (Vec<u8>, f64, usize) {
    entropy_pick_cancellable(settings, candidates, &AtomicBool::new(false)).unwrap()
}

fn entropy_pick_cancellable(
    settings: &Settings,
    candidates: &[Vec<u8>],
    cancelled: &AtomicBool,
) -> Option<(Vec<u8>, f64, usize)> {
    if cancelled.load(Ordering::Relaxed) { return None; }
    let guesses = guess_space(settings);
    if candidates.len() > FULL_SPACE_LIMIT {
        let sampled = sample_guesses(candidates, SAMPLE_SIZE);
        let (guess, _, _) = entropy_best_cancellable(&sampled, &guesses, cancelled)?;
        entropy_best_cancellable(candidates, &[guess], cancelled)
    } else {
        entropy_best_cancellable(candidates, &guesses, cancelled)
    }
}

/// 记忆化键:(剩余深度, 候选集扁平化)。值 None = 该预算下无法证明。
/// 分片共享 memo:跨并行任务全局复用子问题(原串行版的提速关键),
/// 64 片 Mutex 降低争用;缓存是纯函数结果,不影响确定性。
struct SharedMemo {
    shards: Vec<Mutex<HashMap<(usize, Vec<u8>), Option<usize>>>>,
}

impl SharedMemo {
    fn new() -> Self {
        Self {
            shards: (0..64).map(|_| Mutex::new(HashMap::new())).collect(),
        }
    }
    fn get(&self, key: &(usize, Vec<u8>)) -> Option<Option<usize>> {
        let shard = Self::shard_of(key);
        self.shards[shard].lock().unwrap().get(key).copied()
    }
    fn insert(&self, key: (usize, Vec<u8>), val: Option<usize>) {
        let shard = Self::shard_of(&key);
        self.shards[shard].lock().unwrap().insert(key, val);
    }
    fn shard_of(key: &(usize, Vec<u8>)) -> usize {
        // 简单折叠哈希:预算号混入键字节,分布足够均匀即可
        let mut h = key.0 as usize;
        for &b in &key.1 {
            h = h.wrapping_mul(31) + b as usize;
        }
        h % 64
    }
}

fn lookahead_steps(
    cands: &[Vec<u8>],
    guesses: &[Vec<u8>],
    terminal: (u8, u8),
    budget: usize,
    memo: &SharedMemo,
    cancelled: &AtomicBool,
) -> Option<usize> {
    if cancelled.load(Ordering::Relaxed) { return None; }
    if budget == 0 {
        return None;
    }
    if cands.len() == 1 {
        return Some(1); // 已知答案仍需在预算内提交 1 次
    }
    let key = (budget, cands.iter().flat_map(|c| c.iter().copied()).collect::<Vec<u8>>());
    if let Some(cached) = memo.get(&key) {
        return cached;
    }
    let mut best: Option<usize> = None;
    for g in guesses {
        if cancelled.load(Ordering::Relaxed) { return None; }
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
            match lookahead_steps(bucket, guesses, terminal, budget - 1, memo, cancelled) {
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
        // 多候选至少还需两次提交;达到下界后不必继续扫描子问题。
        if best == Some(2) { break; }
    }
    if cancelled.load(Ordering::Relaxed) { return None; }
    memo.insert(key, best);
    best
}

/// 玩 g 后的最坏剩余步数:max(非终局桶 steps(桶, budget-1))。
/// budget=0 直接不可证(与递归入口的预算检查语义一致,usize 下不会减到负)。
fn worst_after_guess(
    g: &[u8],
    cands: &[Vec<u8>],
    guesses: &[Vec<u8>],
    terminal: (u8, u8),
    budget: usize,
    memo: &SharedMemo,
    cancelled: &AtomicBool,
) -> Option<usize> {
    if budget == 0 || cancelled.load(Ordering::Relaxed) {
        return None;
    }
    let mut buckets: HashMap<(u8, u8), Vec<Vec<u8>>> = HashMap::new();
    for c in cands {
        buckets.entry(crate::core::judge::judge(g, c)).or_default().push(c.clone());
    }
    let mut worst = 0usize;
    for (fb, bucket) in &buckets {
        if *fb == terminal {
            continue;
        }
        worst = worst.max(lookahead_steps(bucket, guesses, terminal, budget - 1, memo, cancelled)?);
    }
    Some(worst)
}

/// minimax 前瞻:返回保证最少剩余步数(含本次猜测)的猜测与该步数;
/// 深度预算用尽无法证明 → None;采样 guesses 时只在此搜索范围内优化。
/// 步数语义:steps(C)=1+min_g max_{非终局桶} steps(桶);反馈=(slots,0) 为终局桶;
/// 单候选桶 steps=1(直接提交)。
/// 并行化:根层各猜测独立评估 → rayon 并行;子问题经分片共享 memo 全局复用
/// (原串行版提速关键);min 归约与平手裁决(先枚举序取首个达成者)串行确定,
/// 结果与串行版逐位一致。
#[cfg(test)]
pub(crate) fn lookahead_best(
    candidates: &[Vec<u8>],
    guesses: &[Vec<u8>],
    depth_cap: usize,
) -> Option<(Vec<u8>, usize)> {
    lookahead_best_cancellable(candidates, guesses, depth_cap, &AtomicBool::new(false))
}

fn lookahead_best_cancellable(
    candidates: &[Vec<u8>],
    guesses: &[Vec<u8>],
    depth_cap: usize,
    cancelled: &AtomicBool,
) -> Option<(Vec<u8>, usize)> {
    if cancelled.load(Ordering::Relaxed) { return None; }
    debug_assert!(!candidates.is_empty() && !guesses.is_empty());
    let slots = candidates[0].len() as u8;
    let terminal = (slots, 0);
    let memo = SharedMemo::new();
    let totals: Vec<Option<usize>> = guesses
        .par_iter()
        .map(|g| worst_after_guess(g, candidates, guesses, terminal, depth_cap, &memo, cancelled))
        .collect();
    if cancelled.load(Ordering::Relaxed) { return None; }
    let best = totals.iter().filter_map(|t| *t).map(|w| 1 + w).min()?;
    // 平手裁决:枚举序中第一个达成保证步数的猜测
    let idx = totals.iter().position(|t| t.map_or(false, |w| 1 + w == best))?;
    Some((guesses[idx].clone(), best))
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
    fn lookahead_cannot_submit_a_singleton_after_depth_budget_expires() {
        let guesses = default_space();
        assert_eq!(lookahead_best(&c3(), &guesses, 1), None);
        assert_eq!(lookahead_best(&c3(), &guesses, 2), Some((vec![0, 0, 0, 0], 2)));
    }

    #[test]
    fn sampled_selection_reports_the_full_candidate_worst_bucket() {
        let settings = Settings { colors: 8, slots: 5, repeats: true };
        let candidates = enumerate_space(&settings);
        let (guess, _, reported_worst) = entropy_pick(&settings, &candidates);
        let mut actual_buckets = HashMap::new();
        for candidate in &candidates {
            *actual_buckets.entry(crate::core::judge::judge(&guess, candidate)).or_insert(0) += 1;
        }
        assert_eq!(reported_worst, *actual_buckets.values().max().unwrap());
    }

    #[test]
    fn quick_recommendation_is_actionable_before_lookahead() {
        let settings = Settings::default();
        let candidates = ab_candidates();
        assert!(matches!(
            recommend_quick_for(&settings, &candidates),
            Recommendation::Guess { guess, bound: Bound::Expected { worst_bucket: 5, .. } }
                if guess == vec![3, 4, 4, 1]
        ));
        assert_eq!(recommend_quick_for(&settings, &candidates[..1]), Recommendation::Answer(candidates[0].clone()));
        assert!(matches!(
            recommend_quick_for(&settings, &candidates[..2]),
            Recommendation::Guess { bound: Bound::GuaranteedSteps(2), .. }
        ));
    }

    #[test]
    fn evidence_explains_diagnostic_guesses_and_all_candidate_partitions() {
        let settings = Settings::default();
        let candidates = ab_candidates();
        let rec = recommend_quick_for(&settings, &candidates);
        let evidence = recommendation_evidence(&settings, &candidates, &rec);
        assert_eq!(evidence.candidate_count, 24);
        assert!(!evidence.is_possible_answer);
        assert!(!evidence.sampled_search);
        let guess = match &rec { Recommendation::Guess { guess, .. } => guess, _ => unreachable!() };
        let mut counts = HashMap::new();
        for candidate in &candidates {
            *counts.entry(crate::core::judge::judge(guess, candidate)).or_insert(0usize) += 1;
        }
        let expected = counts.values().map(|&count| (count * count) as f64 / 24.0).sum::<f64>();
        assert!((evidence.expected_remaining - expected).abs() < 1e-12);
        assert_eq!(evidence.worst_bucket, 5);
        assert!((evidence.entropy_bits - 3.173_533).abs() < 1e-4);
    }

    #[test]
    fn evidence_discloses_sampling_only_for_searched_recommendations() {
        let settings = Settings { colors: 8, slots: 5, repeats: true };
        let candidates = enumerate_space(&settings);
        let rec = recommend_quick_for(&settings, &candidates);
        let evidence = recommendation_evidence(&settings, &candidates, &rec);
        assert!(evidence.sampled_search);
        assert!(evidence.is_possible_answer);
        assert_eq!(evidence.candidate_count, 32768);
        assert!(matches!(rec, Recommendation::Guess { bound: Bound::Expected { entropy_bits, worst_bucket }, .. }
            if entropy_bits == evidence.entropy_bits && worst_bucket == evidence.worst_bucket));
        for count in 1..=2 {
            let rec = recommend_quick_for(&settings, &candidates[..count]);
            let evidence = recommendation_evidence(&settings, &candidates[..count], &rec);
            assert!(!evidence.sampled_search);
            assert!(evidence.is_possible_answer);
        }
    }

    #[test]
    fn cancelled_recommendation_never_publishes_a_result() {
        let cancelled = std::sync::atomic::AtomicBool::new(true);
        let settings = Settings::default();
        let candidates = default_space();
        for count in [1, 2, 24, candidates.len()] {
            assert_eq!(recommend_for_cancellable(&settings, &candidates[..count], &cancelled), None);
        }
    }

    #[test]
    fn cancellation_interrupts_search_in_progress() {
        use std::sync::{Arc, Barrier};
        use std::sync::atomic::{AtomicBool, Ordering};
        let settings = Settings { colors: 8, slots: 6, repeats: true };
        let candidates = enumerate_space(&settings);
        let cancelled = Arc::new(AtomicBool::new(false));
        let started = Arc::new(Barrier::new(2));
        let worker_cancelled = cancelled.clone();
        let worker_started = started.clone();
        let worker = std::thread::spawn(move || {
            worker_started.wait();
            recommend_for_cancellable(&settings, &candidates[..30], &worker_cancelled)
        });
        started.wait();
        std::thread::sleep(std::time::Duration::from_millis(10));
        cancelled.store(true, Ordering::Relaxed);
        assert_eq!(worker.join().unwrap(), None);
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
