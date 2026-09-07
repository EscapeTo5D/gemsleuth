//! 全空间枚举 + 过滤 + 求解编排(§4.3)。

use crate::core::judge::judge;
use crate::core::model::Record;
use crate::core::model::Settings;
use crate::core::strategy::{recommend_for, Recommendation};

/// 枚举全部可能答案(§4.3 第 1 步)。
/// repeats=true → colors^slots 个,里程表序(末位变化最快,即 itertools.product 序);
/// repeats=false → 无重复全排列 P(colors, slots),字典序。
/// 要求 settings 已通过 validate(debug_assert)。
pub fn enumerate_space(settings: &Settings) -> Vec<Vec<u8>> {
    debug_assert!(settings.validate().is_ok());
    let colors = settings.colors;
    let slots = settings.slots;
    let mut out = Vec::new();
    if settings.repeats {
        let total = colors.pow(slots as u32);
        out.reserve(total);
        for i in 0..total {
            // v[0] 是最高位:末位变化最快(itertools.product 序)
            let mut v = vec![0u8; slots];
            let mut i = i;
            for pos in (0..slots).rev() {
                v[pos] = (i % colors) as u8;
                i /= colors;
            }
            out.push(v);
        }
    } else {
        out.reserve(colors * (colors - 1).max(1));
        let mut cur = Vec::with_capacity(slots);
        let mut used = vec![false; colors];
        fn dfs(colors: usize, slots: usize, cur: &mut Vec<u8>, used: &mut [bool], out: &mut Vec<Vec<u8>>) {
            if cur.len() == slots {
                out.push(cur.clone());
                return;
            }
            for c in 0..colors {
                if used[c] {
                    continue;
                }
                used[c] = true;
                cur.push(c as u8);
                dfs(colors, slots, cur, used, out);
                cur.pop();
                used[c] = false;
            }
        }
        dfs(colors, slots, &mut cur, &mut used, &mut out);
    }
    out
}

/// 返回使所有 enabled 记录的判定反馈与录入值完全一致的候选(§4.3 第 2 步)。
/// 禁用的记录不参与过滤(F4/F5)。保持枚举序。
pub fn filter_candidates(settings: &Settings, records: &[Record]) -> Vec<Vec<u8>> {
    enumerate_space(settings)
        .into_iter()
        .filter(|cand| {
            records
                .iter()
                .filter(|r| r.enabled)
                .all(|r| judge(&r.guess, cand) == (r.exact, r.partial))
        })
        .collect()
}

/// 求解编排结果(§4.3,GUI 与最终用户依赖)。
#[derive(Debug, Clone, PartialEq)]
pub enum SolveOutcome {
    /// 候选唯一:即最终答案。
    Unique(Vec<u8>),
    /// 多候选并存:附下一步推荐。
    Ambiguous { candidates: Vec<Vec<u8>>, recommendation: Recommendation },
    /// 候选为空:enabled 记录整体矛盾(用 suspect_records 排查)。
    Contradiction,
}

/// 求解编排(§4.3 第 3 步):过滤出全部候选后按数量三分支。
pub fn solve(settings: &Settings, records: &[Record]) -> SolveOutcome {
    let candidates = filter_candidates(settings, records);
    match candidates.len() {
        0 => SolveOutcome::Contradiction,
        1 => SolveOutcome::Unique(candidates.into_iter().next().unwrap()),
        _ => SolveOutcome::Ambiguous {
            recommendation: recommend_for(settings, &candidates),
            candidates,
        },
    }
}

/// 矛盾排查(F5):当前整体矛盾时,返回"禁用后候选恢复非空"的记录下标;非矛盾返回空。
pub fn suspect_records(settings: &Settings, records: &[Record]) -> Vec<usize> {
    if !filter_candidates(settings, records).is_empty() {
        return vec![]; // 整体不矛盾时无嫌疑可谈
    }
    (0..records.len())
        .filter(|&i| {
            records[i].enabled && {
                let mut others = records.to_vec();
                others[i].enabled = false;
                !filter_candidates(settings, &others).is_empty()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::strategy::{Bound, Recommendation};

    #[test]
    fn repeats_space_size() {
        assert_eq!(enumerate_space(&Settings { colors: 6, slots: 4, repeats: true }).len(), 1296);
        assert_eq!(enumerate_space(&Settings { colors: 8, slots: 6, repeats: true }).len(), 262_144);
        assert_eq!(enumerate_space(&Settings { colors: 4, slots: 3, repeats: true }).len(), 64);
    }

    #[test]
    fn repeats_space_odometer_order() {
        let s = enumerate_space(&Settings { colors: 6, slots: 4, repeats: true });
        // 末位变化最快(与 itertools.product 一致;后续策略测试的期望值依赖此序)
        assert_eq!(s[0], vec![0, 0, 0, 0]);
        assert_eq!(s[1], vec![0, 0, 0, 1]);
        assert_eq!(s[6], vec![0, 0, 1, 0]);
        assert_eq!(s[1295], vec![5, 5, 5, 5]);
    }

    #[test]
    fn permutation_space() {
        let s = enumerate_space(&Settings { colors: 6, slots: 4, repeats: false });
        assert_eq!(s.len(), 360); // P(6,4)
        assert_eq!(s[0], vec![0, 1, 2, 3]);
        assert_eq!(s[1], vec![0, 1, 2, 4]);
        assert_eq!(s[359], vec![5, 4, 3, 2]);
        assert!(s.iter().all(|c| c.iter().collect::<std::collections::HashSet<_>>().len() == 4));
    }

    #[test]
    fn permutation_space_8_colors() {
        assert_eq!(enumerate_space(&Settings { colors: 8, slots: 4, repeats: false }).len(), 1680);
    }

    use crate::core::judge::judge;
    use crate::core::model::Record;

    // 真实谜面两条记录(附录 A 金标准的前两条)
    fn ab_records() -> Vec<Record> {
        vec![
            Record::new(vec![3, 1, 2, 0], 1, 0), // 橙蓝紫红
            Record::new(vec![3, 1, 2, 5], 1, 0), // 橙蓝紫绿
        ]
    }

    #[test]
    fn filter_empty_records_is_full_space() {
        let s = Settings::default();
        assert_eq!(filter_candidates(&s, &[]).len(), 1296);
    }

    #[test]
    fn filter_real_two_records_leaves_24() {
        // 已由穷举脚本验证:橙蓝紫红(1,0) + 橙蓝紫绿(1,0) → 恰好 24 个候选
        let s = Settings::default();
        let cands = filter_candidates(&s, &ab_records());
        assert_eq!(cands.len(), 24);
        assert!(cands.iter().any(|c| c == &vec![2, 2, 2, 4])); // 紫紫紫黄
        assert!(cands.iter().any(|c| c == &vec![1, 1, 1, 1]));
        // 枚举序中第一个候选是 [1,1,1,1]
        assert_eq!(cands[0], vec![1, 1, 1, 1]);
    }

    #[test]
    fn filter_respects_all_enabled_records() {
        let s = Settings::default();
        let cands = filter_candidates(&s, &ab_records());
        assert!(cands.iter().all(|c| {
            judge(&[3, 1, 2, 0], c) == (1, 0) && judge(&[3, 1, 2, 5], c) == (1, 0)
        }));
    }

    #[test]
    fn disabled_record_ignored() {
        let s = Settings::default();
        let mut r = Record::new(vec![0, 1, 2, 3], 4, 0); // 声称红蓝紫橙即答案
        r.enabled = false;
        assert_eq!(filter_candidates(&s, &[r]).len(), 1296);
    }

    #[test]
    fn filter_contradiction_is_empty() {
        let s = Settings::default();
        let mut rs = ab_records();
        rs.push(Record::new(vec![0, 1, 2, 3], 4, 0)); // 与前两条矛盾
        assert!(filter_candidates(&s, &rs).is_empty());
    }

    // 金标准:6 条记录唯一解出 紫紫紫黄(附录 A;前 2 条为真实谜面,后 4 条构造补充)
    fn golden_records() -> Vec<Record> {
        vec![
            Record::new(vec![3, 1, 2, 0], 1, 0), // 橙蓝紫红(真实)
            Record::new(vec![3, 1, 2, 5], 1, 0), // 橙蓝紫绿(真实)
            Record::new(vec![0, 1, 2, 3], 1, 0), // 红蓝紫橙
            Record::new(vec![0, 0, 1, 1], 0, 0), // 红红蓝蓝
            Record::new(vec![4, 5, 0, 1], 0, 1), // 黄绿红蓝
            Record::new(vec![2, 4, 2, 2], 2, 2), // 紫黄紫紫
        ]
    }

    fn contradiction_records() -> Vec<Record> {
        let mut rs = golden_records();
        rs.truncate(2);
        rs.push(Record::new(vec![0, 1, 2, 3], 4, 0)); // 声称红蓝紫橙即答案,与前两条矛盾
        rs
    }

    #[test]
    fn golden_puzzle_solves_unique() {
        let oc = solve(&Settings::default(), &golden_records());
        assert_eq!(oc, SolveOutcome::Unique(vec![2, 2, 2, 4])); // 紫紫紫黄
    }

    #[test]
    fn solve_is_deterministic() {
        let s = Settings::default();
        assert_eq!(solve(&s, &golden_records()), solve(&s, &golden_records()));
    }

    #[test]
    fn solve_contradiction_and_suspects() {
        let s = Settings::default();
        let rs = contradiction_records();
        assert_eq!(solve(&s, &rs), SolveOutcome::Contradiction);
        assert_eq!(suspect_records(&s, &rs), vec![2]); // 第 3 条(4,0)是嫌疑
        // 禁用嫌疑记录后恢复为 24 候选的 Ambiguous
        let mut fixed = rs.clone();
        fixed[2].enabled = false;
        match solve(&s, &fixed) {
            SolveOutcome::Ambiguous { candidates, .. } => assert_eq!(candidates.len(), 24),
            other => panic!("应恢复为 Ambiguous,实际 {other:?}"),
        }
    }

    #[test]
    fn solve_empty_records_ambiguous_with_entropy_recommendation() {
        match solve(&Settings::default(), &[]) {
            SolveOutcome::Ambiguous { candidates, recommendation } => {
                assert_eq!(candidates.len(), 1296);
                match recommendation {
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
            other => panic!("应 Ambiguous,实际 {other:?}"),
        }
    }

    #[test]
    fn solve_two_candidates_ambiguous() {
        let rs = vec![
            Record::new(vec![3, 1, 2, 0], 1, 0),
            Record::new(vec![3, 1, 2, 5], 1, 0),
            Record::new(vec![0, 1, 2, 3], 1, 0),
            Record::new(vec![0, 0, 1, 1], 0, 0),
            Record::new(vec![2, 4, 2, 2], 2, 2),
        ];
        match solve(&Settings::default(), &rs) {
            SolveOutcome::Ambiguous { candidates, recommendation } => {
                assert_eq!(candidates, vec![vec![2, 2, 2, 4], vec![4, 2, 2, 2]]);
                assert_eq!(
                    recommendation,
                    Recommendation::Guess { guess: vec![2, 2, 2, 4], bound: Bound::GuaranteedSteps(2) }
                );
            }
            other => panic!("应 Ambiguous,实际 {other:?}"),
        }
    }

    #[test]
    fn permutation_mode_smoke() {
        // 规格 §6 排列模式冒烟:repeats=false 下求解正确性(性质断言)
        // 注:不能选 (1,0) 这类反馈——6 色中猜测外仅剩 2 色,凑不出"只共享 1 色"的 4 互异候选,必然矛盾
        let s = Settings { colors: 6, slots: 4, repeats: false };
        let rs = [Record::new(vec![0, 1, 2, 3], 2, 2)];
        let cands = filter_candidates(&s, &rs);
        assert!(!cands.is_empty());
        assert!(cands.iter().any(|c| c == &vec![1, 0, 2, 3])); // 换位前两个槽即满足 (2,2)
        assert!(cands.iter().all(|c| {
            c.iter().collect::<std::collections::HashSet<_>>().len() == 4  // 无重复
                && judge(&[0, 1, 2, 3], c) == (2, 2)                       // 满足记录
        }));
    }
}
