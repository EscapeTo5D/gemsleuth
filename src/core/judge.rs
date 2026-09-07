//! 反馈判定:标准 Mastermind 多重集计数(§4.2)。
//!
//! exact   = Σ_i [guess[i] == candidate[i]]
//! overlap = Σ_色c min(count(guess,c), count(candidate,c))
//! partial = overlap − exact
//! 复杂度 O(slots²)(slots ≤ 6,无需更巧)。

/// 判定 guess 对 candidate 的反馈:(蓝标 exact, 金标 partial)。
pub fn judge(guess: &[u8], candidate: &[u8]) -> (u8, u8) {
    debug_assert_eq!(guess.len(), candidate.len());
    let exact = guess.iter().zip(candidate).filter(|(g, c)| g == c).count() as u8;
    let mut overlap = 0usize;
    for (i, &color) in guess.iter().enumerate() {
        // 只在该颜色首次出现时统计一次,避免重复累加
        if guess[..i].contains(&color) {
            continue;
        }
        let g = guess.iter().filter(|&&x| x == color).count();
        let c = candidate.iter().filter(|&&x| x == color).count();
        overlap += g.min(c);
    }
    (exact, (overlap as u8) - exact)
}

#[cfg(test)]
mod tests {
    use super::judge;

    #[test]
    fn all_exact() {
        assert_eq!(judge(&[1, 2, 3, 4], &[1, 2, 3, 4]), (4, 0));
    }

    #[test]
    fn all_miss() {
        assert_eq!(judge(&[0, 0, 0, 0], &[1, 1, 1, 1]), (0, 0));
    }

    #[test]
    fn repeated_multiset_counting() {
        // 规格 §6 示例:guess=黄黄紫蓝 vs candidate=紫紫紫黄。
        // 按 §4.2 公式复核为 (1,1)(规格中 (0,2) 为笔误,见计划偏差表):
        // exact=1(第 3 位紫),overlap=min(黄2,黄1)+min(紫1,紫3)=2,partial=1。
        // 颜色索引:0红 1蓝 2紫 3橙 4黄 5绿。
        assert_eq!(judge(&[4, 4, 2, 1], &[2, 2, 2, 4]), (1, 1));
        // 重复宝石按较少一侧计数:紫黄紫紫 vs 紫紫紫黄
        assert_eq!(judge(&[2, 4, 2, 2], &[2, 2, 2, 4]), (2, 2));
    }

    #[test]
    fn real_puzzle_records() {
        // 真实谜面两条记录(规格 §5.1 GUI 草图)对唯一解 紫紫紫黄 的反馈——锁死判定语义
        assert_eq!(judge(&[3, 1, 2, 0], &[2, 2, 2, 4]), (1, 0)); // 橙蓝紫红
        assert_eq!(judge(&[3, 1, 2, 5], &[2, 2, 2, 4]), (1, 0)); // 橙蓝紫绿
    }

    #[test]
    fn partial_only_and_three_slots() {
        assert_eq!(judge(&[0, 0, 0, 0], &[0, 0, 0, 1]), (3, 0));
        assert_eq!(judge(&[0, 1, 2], &[2, 0, 1]), (0, 3));
    }
}
