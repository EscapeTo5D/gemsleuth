//! Generate average-oriented and minimax-oriented policies; no global optimality claim.
use gemsleuth::{Settings, enumerate_space, judge};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Copy, Debug)]
struct Node {
    guess: u16,
    total: usize,
    worst: usize,
}
struct Builder {
    feedback: Vec<u8>,
    memo: HashMap<Vec<u16>, Node>,
    average: bool,
}
impl Builder {
    fn partitions(&self, candidates: &[u16], guess: u16) -> Vec<Vec<u16>> {
        let mut buckets = vec![Vec::new(); 25];
        for &answer in candidates {
            buckets[self.feedback[guess as usize * 1296 + answer as usize] as usize].push(answer);
        }
        buckets
    }
    fn solve(&mut self, c: &[u16]) -> Node {
        if let Some(&node) = self.memo.get(c) {
            return node;
        }
        if c.len() == 1 {
            return Node {
                guess: c[0],
                total: 1,
                worst: 1,
            };
        }
        let exact = self.average && c.len() <= 8;
        let mut ranked = Vec::new();
        for guess in 0..1296u16 {
            let mut counts = [0usize; 25];
            for &answer in c {
                counts[self.feedback[guess as usize * 1296 + answer as usize] as usize] += 1;
            }
            if *counts.iter().max().unwrap() == c.len() {
                continue;
            }
            let score = if self.average {
                counts
                    .iter()
                    .filter(|&&n| n > 0)
                    .map(|&n| n as f64 * (n as f64).log2())
                    .sum()
            } else {
                *counts.iter().max().unwrap() as f64
            };
            ranked.push((score, !c.contains(&guess), guess));
        }
        ranked.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if self.average && c.len() == 1296 {
            // All five opening multiplicity classes, rather than two equivalent
            // color/position permutations of the same opening.
            let mut seen = std::collections::HashSet::new();
            ranked.retain(|&(_, _, mut guess)| {
                let mut counts = [0u8; 6];
                for _ in 0..4 {
                    counts[(guess % 6) as usize] += 1;
                    guess /= 6;
                }
                counts.sort_unstable();
                seen.insert(counts)
            });
        }
        let mut best: Option<Node> = None;
        // Small sets: exact expected-cost optimization over every legal guess.
        // Large sets: compare two entropy choices, or take greedy minimax.
        let take = if exact || (self.average && c.len() == 1296) {
            ranked.len()
        } else if self.average {
            2
        } else {
            1
        };
        for &(_, _, guess) in ranked.iter().take(take) {
            let buckets = self.partitions(c, guess);
            let lower = c.len()
                + buckets
                    .iter()
                    .enumerate()
                    .filter(|(f, b)| *f != 20 && !b.is_empty())
                    .map(|(_, b)| 2 * b.len() - 1)
                    .sum::<usize>();
            if self.average && best.is_some_and(|b| lower >= b.total) {
                continue;
            }
            let mut node = Node {
                guess,
                total: c.len(),
                worst: 1,
            };
            for (feedback, bucket) in buckets.iter().enumerate() {
                if feedback == 20 || bucket.is_empty() {
                    continue;
                }
                let child = self.solve(bucket);
                node.total += child.total;
                node.worst = node.worst.max(child.worst + 1);
                if self.average && best.is_some_and(|b| node.total >= b.total) {
                    break;
                }
            }
            if best.is_none_or(|b| node.total < b.total) {
                best = Some(node);
            }
        }
        let best = best.unwrap();
        self.memo.insert(c.to_vec(), best);
        best
    }
    fn collect(&self, c: &[u16], rows: &mut BTreeMap<Vec<u16>, String>) {
        if c.len() <= 1 || rows.contains_key(c) {
            return;
        }
        let node = self.memo[c];
        rows.insert(
            c.to_vec(),
            format!(
                "{};{};{}",
                c.iter().map(u16::to_string).collect::<Vec<_>>().join(","),
                node.guess,
                node.worst
            ),
        );
        for (f, b) in self.partitions(c, node.guess).iter().enumerate() {
            if f != 20 && !b.is_empty() {
                self.collect(b, rows);
            }
        }
    }
}
fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/heuristic-policy".into());
    std::fs::create_dir_all(&out).unwrap();
    let space = enumerate_space(&Settings::default());
    let feedback: Vec<_> = space
        .iter()
        .flat_map(|g| {
            space.iter().map(move |a| {
                let (e, p) = judge(g, a);
                e * 5 + p
            })
        })
        .collect();
    for average in [false, true] {
        let started = std::time::Instant::now();
        let mut builder = Builder {
            feedback: feedback.clone(),
            memo: HashMap::new(),
            average,
        };
        let candidates: Vec<_> = (0..1296).collect();
        let result = builder.solve(&candidates);
        let mut rows = BTreeMap::new();
        builder.collect(&candidates, &mut rows);
        let name = if average { "average" } else { "worst" };
        std::fs::write(
            format!("{out}/policy_{name}.txt"),
            rows.values().cloned().collect::<Vec<_>>().join("\n") + "\n",
        )
        .unwrap();
        println!(
            "{name}: average={:.6} worst={} total={} nodes={} elapsed={:?}",
            result.total as f64 / 1296.0,
            result.worst,
            result.total,
            rows.len(),
            started.elapsed()
        );
    }
}
