//! 分层自适应推荐(§4.4):熵最大化 / 精确前瞻。

use std::collections::HashMap;

use crate::core::model::Settings;
use crate::core::solver::enumerate_space;

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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::core::model::Record;
    use crate::core::solver::filter_candidates;

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
}
