//! Offline policy for the default game. Exact candidate-set lookup; other states fall back to search.
use super::{
    model::Settings,
    strategy::{Bound, Recommendation},
};
use std::{collections::HashMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StrategyMode {
    #[default]
    Average,
    Worst,
}

type Table = HashMap<Vec<u16>, (u16, usize)>;
fn parse_table(text: &str) -> Table {
    text.lines()
        .map(|line| {
            let mut fields = line.split(';');
            let key = fields
                .next()
                .unwrap()
                .split(',')
                .map(|s| s.parse().unwrap())
                .collect();
            let guess = fields.next().unwrap().parse().unwrap();
            let steps = fields.next().unwrap().parse().unwrap();
            (key, (guess, steps))
        })
        .collect()
}

pub fn lookup_mode(
    settings: &Settings,
    candidates: &[Vec<u8>],
    mode: StrategyMode,
) -> Option<Recommendation> {
    static AVERAGE: OnceLock<Table> = OnceLock::new();
    static WORST: OnceLock<Table> = OnceLock::new();
    let table = match mode {
        StrategyMode::Average => {
            AVERAGE.get_or_init(|| parse_table(include_str!("../../assets/policy_average.txt")))
        }
        StrategyMode::Worst => {
            WORST.get_or_init(|| parse_table(include_str!("../../assets/policy_worst.txt")))
        }
    };
    lookup_table(settings, candidates, table)
}

fn code_id(code: &[u8]) -> u16 {
    code.iter().fold(0, |id, &v| id * 6 + v as u16)
}

fn decode(mut id: u16) -> Vec<u8> {
    let mut code = vec![0; 4];
    for v in code.iter_mut().rev() {
        *v = (id % 6) as u8;
        id /= 6;
    }
    code
}

pub fn lookup(settings: &Settings, candidates: &[Vec<u8>]) -> Option<Recommendation> {
    static TABLE: OnceLock<Table> = OnceLock::new();
    lookup_table(
        settings,
        candidates,
        TABLE.get_or_init(|| parse_table(include_str!("../../assets/policy_6x4.txt"))),
    )
}

fn lookup_table(
    settings: &Settings,
    candidates: &[Vec<u8>],
    table: &Table,
) -> Option<Recommendation> {
    if *settings != Settings::default() || candidates.is_empty() {
        return None;
    }
    if candidates
        .iter()
        .any(|c| c.len() != 4 || c.iter().any(|&v| v >= 6))
    {
        return None;
    }
    if candidates.len() == 1 {
        return Some(Recommendation::Answer(candidates[0].clone()));
    }
    let mut key: Vec<_> = candidates.iter().map(|c| code_id(c)).collect();
    key.sort_unstable();
    table
        .get(&key)
        .map(|&(guess, steps)| Recommendation::Guess {
            guess: decode(guess),
            bound: Bound::GuaranteedSteps(steps),
        })
}

/// Independently replay every default-game answer using public judge and filtering,
/// without optimizer feedback matrices, cost caches, or lower bounds.
pub fn verify_policy(text: &str) -> Result<(usize, usize), String> {
    let mut table = Table::new();
    for line in text.lines() {
        let fields: Vec<_> = line.split(';').collect();
        if fields.len() != 3 {
            return Err("invalid policy row".into());
        }
        let c: Vec<u16> = fields[0]
            .split(',')
            .map(|s| s.parse().map_err(|_| "invalid candidate"))
            .collect::<Result<_, _>>()?;
        let guess: u16 = fields[1].parse().map_err(|_| "invalid guess")?;
        let steps: usize = fields[2].parse().map_err(|_| "invalid steps")?;
        if c.is_empty()
            || c.iter().any(|&id| id >= 1296)
            || c.windows(2).any(|w| w[0] >= w[1])
            || guess >= 1296
            || steps == 0
            || steps > 1296
        {
            return Err("invalid policy values".into());
        }
        if table.insert(c, (guess, steps)).is_some() {
            return Err("duplicate policy state".into());
        }
    }
    let settings = Settings::default();
    let space = crate::enumerate_space(&settings);
    let (mut total, mut worst) = (0, 0);
    for answer in &space {
        let mut candidates = space.clone();
        let mut promised = usize::MAX;
        for turn in 1..=1296 {
            let guess = match lookup_table(&settings, &candidates, &table)
                .ok_or("missing policy branch")?
            {
                Recommendation::Answer(g) => g,
                Recommendation::Guess {
                    guess,
                    bound: Bound::GuaranteedSteps(steps),
                } => {
                    let next_promise = turn + steps - 1;
                    if next_promise > promised {
                        return Err("inconsistent step guarantee".into());
                    }
                    promised = next_promise;
                    guess
                }
                _ => return Err("unverified branch".into()),
            };
            if turn > promised {
                return Err("step guarantee exceeded".into());
            }
            let feedback = crate::judge(&guess, answer);
            if feedback == (4, 0) {
                total += turn;
                worst = worst.max(turn);
                break;
            }
            let old = candidates.len();
            candidates.retain(|c| crate::judge(&guess, c) == feedback);
            if candidates.is_empty() || candidates.len() >= old || turn == 1296 {
                return Err("policy does not solve every answer".into());
            }
        }
    }
    Ok((total, worst))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{enumerate_space, judge};

    #[test]
    fn independent_verifier_checks_all_answers_and_rejects_false_guarantees() {
        let text = include_str!("../../assets/policy_average.txt");
        assert_eq!(verify_policy(text).unwrap(), (5625, 6));
        let mut rows: Vec<_> = text.lines().map(str::to_string).collect();
        let root = rows
            .iter_mut()
            .find(|r| r.split(';').next().unwrap().split(',').count() == 1296)
            .unwrap();
        *root = format!("{};1", root.rsplit_once(';').unwrap().0);
        assert!(verify_policy(&rows.join("\n")).is_err());
        assert!(verify_policy("invalid").is_err());
        assert!(verify_policy("").is_err());
    }

    #[test]
    fn both_strategies_solve_every_answer_and_match_reported_totals() {
        let settings = Settings::default();
        let space = enumerate_space(&settings);
        for (mode, expected_total, expected_worst) in [
            (StrategyMode::Average, 5625, 6),
            (StrategyMode::Worst, 5801, 5),
        ] {
            let mut total = 0;
            let mut worst = 0;
            for answer in &space {
                let mut candidates = space.clone();
                let mut promised = expected_worst;
                let mut solved = false;
                for turn in 1..=expected_worst {
                    let guess = match lookup_mode(&settings, &candidates, mode)
                        .expect("uncovered branch")
                    {
                        Recommendation::Answer(g) => g,
                        Recommendation::Guess {
                            guess,
                            bound: Bound::GuaranteedSteps(steps),
                        } => {
                            assert!(turn + steps - 1 <= promised);
                            promised = turn + steps - 1;
                            guess
                        }
                        _ => panic!("missing guarantee"),
                    };
                    let feedback = judge(&guess, answer);
                    if feedback == (4, 0) {
                        assert!(turn <= promised);
                        total += turn;
                        worst = worst.max(turn);
                        solved = true;
                        break;
                    }
                    candidates.retain(|c| judge(&guess, c) == feedback);
                }
                assert!(solved, "{mode:?} failed on {answer:?}");
            }
            assert_eq!((total, worst), (expected_total, expected_worst));
        }
    }

    #[test]
    fn every_answer_is_solved_within_the_advertised_bound() {
        let settings = Settings::default();
        let space = enumerate_space(&settings);
        let mut histogram = [0usize; 16];
        for answer in &space {
            let mut candidates = space.clone();
            let mut promised = usize::MAX;
            for turn in 1..16 {
                let rec = lookup(&settings, &candidates)
                    .expect("policy must cover every reachable state");
                let guess = match rec {
                    Recommendation::Answer(g) => g,
                    Recommendation::Guess {
                        guess,
                        bound: Bound::GuaranteedSteps(steps),
                    } => {
                        assert!(turn + steps - 1 <= promised);
                        promised = turn + steps - 1;
                        guess
                    }
                    _ => panic!("unverified policy entry"),
                };
                let feedback = judge(answer, &guess);
                if feedback == (4, 0) {
                    assert!(turn <= promised);
                    histogram[turn] += 1;
                    break;
                }
                candidates.retain(|candidate| judge(candidate, &guess) == feedback);
            }
        }
        assert_eq!(histogram.iter().sum::<usize>(), 1296);
        assert_eq!(
            histogram,
            [0, 1, 4, 29, 292, 910, 60, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        println!("verified policy histogram: {histogram:?}");
    }

    #[test]
    fn unsupported_settings_and_unknown_states_fall_back() {
        assert!(
            lookup(
                &Settings {
                    repeats: false,
                    ..Settings::default()
                },
                &[vec![0, 1, 2, 3]]
            )
            .is_none()
        );
        assert!(lookup(&Settings::default(), &[]).is_none());
        assert!(lookup(&Settings::default(), &[vec![8, 0, 0, 0]]).is_none());
    }
}
