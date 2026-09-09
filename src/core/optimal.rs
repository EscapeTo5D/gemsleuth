//! Offline exact expected-submission optimizer.
use crate::{Settings, enumerate_space, judge};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub guess: u16,
    pub total: usize,
    pub worst: usize,
}
#[derive(Clone, Copy, Debug)]
struct Entry {
    lower: usize,
    upper: Option<Node>,
}
#[derive(Debug)]
pub struct Interrupted;

pub struct Optimizer {
    pub settings: Settings,
    space: Vec<Vec<u8>>,
    feedback: Vec<u8>,
    nonterminal_branches: usize,
    root_guesses: Vec<u16>,
    memo: HashMap<Vec<u16>, Entry>,
    tightened: HashSet<Vec<u16>>,
    pub deadline: Option<Instant>,
    pub calls: u64,
    last_progress: Instant,
}

impl Optimizer {
    pub fn new(settings: Settings) -> Self {
        assert!(
            settings.repeats
                && (1..=4).contains(&settings.slots)
                && (1..=6).contains(&settings.colors)
        );
        let space = enumerate_space(&settings);
        let feedback = space
            .iter()
            .flat_map(|g| {
                space.iter().map(move |a| {
                    let (e, p) = judge(g, a);
                    e * 5 + p
                })
            })
            .collect();
        // The full universe is invariant under every color and position permutation.
        // Only at the full root may guesses with the same multiplicities be merged.
        let mut classes = HashSet::new();
        let root_guesses = space
            .iter()
            .enumerate()
            .filter_map(|(id, g)| {
                let mut counts = vec![0usize; settings.colors];
                for &color in g {
                    counts[color as usize] += 1;
                }
                counts.sort_unstable();
                classes.insert(counts).then_some(id as u16)
            })
            .collect();
        Self {
            settings,
            space,
            feedback,
            root_guesses,
            nonterminal_branches: (settings.slots + 1) * (settings.slots + 2) / 2 - 2,
            memo: HashMap::new(),
            tightened: HashSet::new(),
            deadline: None,
            calls: 0,
            last_progress: Instant::now(),
        }
    }
    pub fn root(&self) -> Vec<u16> {
        (0..self.space.len() as u16).collect()
    }
    pub fn entries(&self) -> usize {
        self.memo.len()
    }
    fn theoretical_lower(&self, mut n: usize) -> usize {
        let (mut capacity, mut depth, mut cost) = (1usize, 1usize, 0usize);
        while n > 0 {
            let take = capacity.min(n);
            cost += depth * take;
            n -= take;
            capacity = capacity.saturating_mul(self.nonterminal_branches).max(1);
            depth += 1;
        }
        cost
    }
    fn lower(&self, c: &[u16]) -> usize {
        self.memo
            .get(c)
            .map_or_else(|| self.theoretical_lower(c.len()), |e| e.lower)
    }
    pub fn bounds(&self, c: &[u16]) -> (usize, Option<Node>) {
        (self.lower(c), self.memo.get(c).and_then(|e| e.upper))
    }
    pub fn is_proven(&self, c: &[u16]) -> bool {
        let (lower, upper) = self.bounds(c);
        upper.is_some_and(|node| lower == node.total)
    }
    fn children(&self, c: &[u16], guess: u16) -> Vec<Vec<u16>> {
        let mut buckets = vec![Vec::new(); 25];
        for &id in c {
            let f = self.feedback[guess as usize * self.space.len() + id as usize] as usize;
            if f != self.settings.slots * 5 {
                buckets[f].push(id);
            }
        }
        buckets.retain(|b| !b.is_empty());
        buckets
    }
    fn tick(&mut self) -> Result<(), Interrupted> {
        self.calls += 1;
        if self.calls % 1024 == 1 {
            let now = Instant::now();
            if self.deadline.is_some_and(|d| now >= d) {
                return Err(Interrupted);
            }
            if now.duration_since(self.last_progress).as_secs() >= 10 {
                let (lower, upper) = self.bounds(&self.root());
                eprintln!(
                    "calls={} entries={} root_lower={} root_upper={:?}",
                    self.calls,
                    self.memo.len(),
                    lower,
                    upper.map(|n| n.total)
                );
                self.last_progress = now;
            }
        }
        Ok(())
    }
    fn save_upper(&mut self, c: &[u16], node: Node) {
        let lower = self.lower(c);
        let entry = self
            .memo
            .entry(c.to_vec())
            .or_insert(Entry { lower, upper: None });
        if entry.upper.is_none_or(|old| node.total < old.total) {
            entry.upper = Some(node);
        }
        assert!(entry.lower <= entry.upper.unwrap().total);
    }

    fn tighten_one_ply(&mut self, c: &[u16]) {
        if c.len() <= 2 || self.is_proven(c) || !self.tightened.insert(c.to_vec()) {
            return;
        }
        let size_bounds: Vec<_> = (0..=c.len()).map(|n| self.theoretical_lower(n)).collect();
        let mut best = usize::MAX;
        for guess in 0..self.space.len() {
            let mut counts = [0usize; 25];
            for &id in c {
                counts[self.feedback[guess * self.space.len() + id as usize] as usize] += 1;
            }
            if counts.iter().any(|&n| n == c.len()) {
                continue;
            }
            counts[self.settings.slots * 5] = 0;
            let lower = c.len() + counts.iter().map(|&n| size_bounds[n]).sum::<usize>();
            best = best.min(lower);
        }
        let lower = self.lower(c).max(best);
        let entry = self
            .memo
            .entry(c.to_vec())
            .or_insert(Entry { lower, upper: None });
        entry.lower = lower;
        assert!(entry.upper.is_none_or(|n| lower <= n.total));
    }

    /// Exact optimum if it is strictly below `limit`; None proves F(C) >= limit.
    /// On interruption only completed lower-bound proofs and feasible strategies survive.
    pub fn solve(&mut self, c: &[u16], limit: usize) -> Result<Option<Node>, Interrupted> {
        self.tick()?;
        assert!(!c.is_empty());
        if self.lower(c) >= limit {
            return Ok(None);
        }
        if c.len() <= 2 {
            let node = Node {
                guess: c[0],
                total: c.len() * 2 - 1,
                worst: c.len(),
            };
            self.memo.insert(
                c.to_vec(),
                Entry {
                    lower: node.total,
                    upper: Some(node),
                },
            );
            return Ok((node.total < limit).then_some(node));
        }
        let prior = self.memo.get(c).and_then(|e| e.upper);
        if let Some(node) = prior {
            if self.lower(c) == node.total {
                return Ok(Some(node));
            }
        }
        let mut best = prior.map_or(limit, |n| n.total.min(limit));
        let guesses: Vec<u16> = if c.len() == self.space.len() {
            self.root_guesses.clone()
        } else {
            self.root()
        };
        let mut ranked = Vec::new();
        let mut signatures = HashSet::new();
        for guess in guesses {
            let mut children = self.children(c, guess);
            if children.iter().any(|b| b.len() == c.len()) {
                continue;
            }
            let lower = c.len() + children.iter().map(|b| self.lower(b)).sum::<usize>();
            if lower >= best {
                continue;
            }
            if c.len() <= 32 {
                // Labels do not affect future cost. Keep terminal outcomes separate by omission.
                children.sort_unstable();
                if !signatures.insert(children.clone()) {
                    continue;
                }
            }
            let tie: usize = children.iter().map(|b| b.len() * b.len()).sum();
            ranked.push((lower, tie, guess, children));
        }
        ranked.sort_unstable_by_key(|(lower, tie, guess, _)| {
            (*lower, *tie, !c.contains(guess), *guess)
        });
        let minimum = ranked.iter().map(|r| r.0).min().unwrap_or(best).min(best);
        let current_lower = self.lower(c).max(minimum);
        self.memo
            .entry(c.to_vec())
            .or_insert(Entry {
                lower: current_lower,
                upper: prior,
            })
            .lower = current_lower;
        for (_, _, guess, mut children) in ranked {
            self.tick()?;
            if c.len() > 32 {
                for child in &children {
                    self.tighten_one_ply(child);
                }
            }
            let mut remaining: usize = children.iter().map(|b| self.lower(b)).sum();
            if c.len() + remaining >= best {
                continue;
            }
            children.sort_unstable_by_key(|b| std::cmp::Reverse(b.len()));
            let (mut cost, mut worst, mut feasible) = (c.len(), 1, true);
            for child in &children {
                remaining -= self.lower(child);
                if cost + remaining >= best {
                    feasible = false;
                    break;
                }
                let allowance = best - cost - remaining;
                if let Some(node) = self.solve(child, allowance)? {
                    cost += node.total;
                    worst = worst.max(1 + node.worst);
                } else {
                    feasible = false;
                    break;
                }
            }
            if feasible && cost < best {
                best = cost;
                self.save_upper(
                    c,
                    Node {
                        guess,
                        total: cost,
                        worst,
                    },
                );
            }
        }
        let entry = self.memo.get_mut(c).unwrap();
        entry.lower = entry.lower.max(best);
        if let Some(node) = entry.upper {
            assert!(entry.lower <= node.total);
        }
        Ok(entry.upper.filter(|n| n.total < limit))
    }

    pub fn seed_policy(&mut self, text: &str) -> Result<(), String> {
        let mut guesses = HashMap::new();
        for line in text.lines() {
            let fields: Vec<_> = line.split(';').collect();
            if fields.len() != 3 {
                return Err("invalid policy row".into());
            }
            let c: Vec<u16> = fields[0]
                .split(',')
                .map(|s| s.parse().map_err(|_| "invalid id"))
                .collect::<Result<_, _>>()?;
            let guess: u16 = fields[1].parse().map_err(|_| "invalid guess")?;
            if guess as usize >= self.space.len() {
                return Err("guess out of range".into());
            }
            guesses.insert(c, guess);
        }
        self.seed_visit(&self.root(), &guesses)?;
        Ok(())
    }
    fn seed_visit(&mut self, c: &[u16], guesses: &HashMap<Vec<u16>, u16>) -> Result<Node, String> {
        let guess = if c.len() == 1 {
            c[0]
        } else {
            *guesses.get(c).ok_or("incomplete policy")?
        };
        let mut node = Node {
            guess,
            total: c.len(),
            worst: 1,
        };
        for child in self.children(c, guess) {
            if child.len() == c.len() {
                return Err("policy makes no progress".into());
            }
            let next = self.seed_visit(&child, guesses)?;
            node.total += next.total;
            node.worst = node.worst.max(next.worst + 1);
        }
        self.save_upper(c, node);
        Ok(node)
    }
    pub fn export_policy(&self) -> Result<(String, Node), String> {
        let mut rows = BTreeMap::new();
        let root = self.export_visit(&self.root(), &mut rows)?;
        Ok((
            rows.values().cloned().collect::<Vec<_>>().join("\n") + "\n",
            root,
        ))
    }

    /// Optimize existing reachable subtrees first, producing a useful incumbent before
    /// exploring completely different root guesses. This does not exclude any guesses.
    pub fn refine_incumbent(&mut self) -> Result<(), Interrupted> {
        let mut rows = BTreeMap::new();
        self.export_visit(&self.root(), &mut rows)
            .expect("incumbent must be complete");
        let mut states: Vec<_> = rows
            .into_keys()
            .filter(|c| c.len() > 2 && c.len() <= 64)
            .collect();
        states.sort_unstable_by(|a, b| a.len().cmp(&b.len()).then(a.cmp(b)));
        for c in states {
            let upper = self.memo[&c].upper.unwrap();
            self.solve(&c, upper.total + 1)?;
        }
        Ok(())
    }
    fn export_visit(
        &self,
        c: &[u16],
        rows: &mut BTreeMap<Vec<u16>, String>,
    ) -> Result<Node, String> {
        let guess = if c.len() == 1 {
            c[0]
        } else {
            self.memo
                .get(c)
                .and_then(|e| e.upper)
                .ok_or("missing strategy")?
                .guess
        };
        let mut node = Node {
            guess,
            total: c.len(),
            worst: 1,
        };
        for child in self.children(c, guess) {
            if child.len() == c.len() {
                return Err("no progress".into());
            }
            let next = self.export_visit(&child, rows)?;
            node.total += next.total;
            node.worst = node.worst.max(next.worst + 1);
        }
        if c.len() > 1 {
            rows.insert(
                c.to_vec(),
                format!(
                    "{};{};{}",
                    c.iter().map(u16::to_string).collect::<Vec<_>>().join(","),
                    guess,
                    node.worst
                ),
            );
        }
        Ok(node)
    }

    pub fn save_checkpoint(&self, path: &std::path::Path) -> std::io::Result<()> {
        let mut bytes = b"GSEXACT1".to_vec();
        bytes.extend([self.settings.colors as u8, self.settings.slots as u8]);
        bytes.extend((self.memo.len() as u64).to_le_bytes());
        for (c, e) in &self.memo {
            bytes.extend((c.len() as u16).to_le_bytes());
            for id in c {
                bytes.extend(id.to_le_bytes());
            }
            bytes.extend((e.lower as u32).to_le_bytes());
            bytes.push(e.upper.is_some() as u8);
            if let Some(n) = e.upper {
                bytes.extend(n.guess.to_le_bytes());
                bytes.extend((n.total as u32).to_le_bytes());
                bytes.extend((n.worst as u16).to_le_bytes());
            }
        }
        bytes.extend(checksum(&bytes).to_le_bytes());
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, &bytes)?;
        std::fs::rename(temp, path)
    }

    /// Checkpoints contain trusted conclusions of this solver, not external proof certificates.
    /// Validate format/checksum/settings before replacing any live state.
    pub fn load_checkpoint(&mut self, path: &std::path::Path) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        if bytes.len() < 26 || &bytes[..8] != b"GSEXACT1" {
            return Err("invalid checkpoint".into());
        }
        let (data, tail) = bytes.split_at(bytes.len() - 8);
        if checksum(data) != u64::from_le_bytes(tail.try_into().unwrap()) {
            return Err("checkpoint checksum mismatch".into());
        }
        if data[8..10] != [self.settings.colors as u8, self.settings.slots as u8] {
            return Err("checkpoint rules mismatch".into());
        }
        let mut cursor = 10;
        fn read<const N: usize>(data: &[u8], cursor: &mut usize) -> Result<[u8; N], String> {
            let value = data
                .get(*cursor..*cursor + N)
                .ok_or("truncated checkpoint")?;
            *cursor += N;
            Ok(value.try_into().unwrap())
        }
        let count = u64::from_le_bytes(read(data, &mut cursor)?) as usize;
        if count > data.len() / 9 {
            return Err("invalid entry count".into());
        }
        let mut memo = HashMap::new();
        for _ in 0..count {
            let len = u16::from_le_bytes(read(data, &mut cursor)?) as usize;
            if len == 0 || len > self.space.len() {
                return Err("invalid candidates".into());
            }
            let mut c = Vec::with_capacity(len);
            for _ in 0..len {
                c.push(u16::from_le_bytes(read(data, &mut cursor)?));
            }
            if c.iter().any(|&i| i as usize >= self.space.len())
                || c.windows(2).any(|w| w[0] >= w[1])
            {
                return Err("invalid candidate ids".into());
            }
            let lower = u32::from_le_bytes(read(data, &mut cursor)?) as usize;
            let upper = match read::<1>(data, &mut cursor)?[0] {
                0 => None,
                1 => Some(Node {
                    guess: u16::from_le_bytes(read(data, &mut cursor)?),
                    total: u32::from_le_bytes(read(data, &mut cursor)?) as usize,
                    worst: u16::from_le_bytes(read(data, &mut cursor)?) as usize,
                }),
                _ => return Err("invalid upper bound flag".into()),
            };
            if lower < self.theoretical_lower(len)
                || lower > len * (len + 1) / 2
                || upper.is_some_and(|n| {
                    n.total < lower
                        || n.guess as usize >= self.space.len()
                        || n.worst == 0
                        || n.worst > len
                })
            {
                return Err("invalid bounds".into());
            }
            if memo.insert(c, Entry { lower, upper }).is_some() {
                return Err("duplicate state".into());
            }
        }
        if cursor != data.len() {
            return Err("unexpected trailing data".into());
        }
        self.memo = memo;
        self.tightened.clear();
        Ok(())
    }
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Settings, enumerate_space, judge};
    use std::collections::HashMap;

    // Independent exhaustive oracle: no bounds, symmetries, or optimized feedback table.
    fn brute(space: &[Vec<u8>], c: &[u16], memo: &mut HashMap<Vec<u16>, usize>) -> usize {
        if c.len() == 1 {
            return 1;
        }
        if let Some(&cost) = memo.get(c) {
            return cost;
        }
        let mut best = usize::MAX;
        for guess in space {
            let mut buckets = HashMap::<_, Vec<u16>>::new();
            for &id in c {
                let f = judge(guess, &space[id as usize]);
                if f.0 as usize != guess.len() {
                    buckets.entry(f).or_default().push(id);
                }
            }
            if buckets.values().any(|b| b.len() == c.len()) {
                continue;
            }
            let cost = c.len()
                + buckets
                    .values()
                    .map(|b| brute(space, b, memo))
                    .sum::<usize>();
            best = best.min(cost);
        }
        memo.insert(c.to_vec(), best);
        best
    }

    #[test]
    fn exact_search_matches_unpruned_oracle_for_every_small_subset() {
        let s = Settings {
            colors: 3,
            slots: 2,
            repeats: true,
        };
        let space = enumerate_space(&s);
        let mut oracle = HashMap::new();
        let mut solver = Optimizer::new(s);
        for mask in 1usize..(1 << space.len()) {
            let c: Vec<_> = (0..space.len())
                .filter(|i| mask & (1 << i) != 0)
                .map(|i| i as u16)
                .collect();
            let expected = brute(&space, &c, &mut oracle);
            solver.tighten_one_ply(&c);
            assert!(solver.lower(&c) <= expected);
            let result = solver.solve(&c, 1000).unwrap().unwrap();
            assert_eq!(result.total, expected, "{c:?}");
        }
    }

    #[test]
    fn cost_threshold_is_strict_and_an_interruption_is_not_a_proof() {
        let s = Settings {
            colors: 3,
            slots: 2,
            repeats: true,
        };
        let mut solver = Optimizer::new(s);
        let c: Vec<_> = (0..9).collect();
        let optimum = brute(&enumerate_space(&s), &c, &mut HashMap::new());
        assert!(solver.solve(&c, optimum).unwrap().is_none());
        assert_eq!(
            solver.solve(&c, optimum + 1).unwrap().unwrap().total,
            optimum
        );
        let mut interrupted = Optimizer::new(s);
        interrupted.deadline = Some(std::time::Instant::now());
        assert!(interrupted.solve(&c, 1000).is_err());
        assert!(!interrupted.is_proven(&c));
    }

    #[test]
    fn checkpoints_resume_proofs_and_reject_corruption() {
        let settings = Settings {
            colors: 3,
            slots: 2,
            repeats: true,
        };
        let mut solver = Optimizer::new(settings);
        let root = solver.root();
        let node = solver.solve(&root, 1000).unwrap().unwrap();
        let path =
            std::env::temp_dir().join(format!("gemsleuth-exact-{}.checkpoint", std::process::id()));
        solver.save_checkpoint(&path).unwrap();
        // Replacing an existing snapshot must also work on Windows.
        solver.save_checkpoint(&path).unwrap();
        let mut resumed = Optimizer::new(settings);
        resumed.load_checkpoint(&path).unwrap();
        assert!(resumed.is_proven(&root));
        assert_eq!(
            resumed.solve(&root, 1000).unwrap().unwrap().total,
            node.total
        );
        let (policy, exported) = resumed.export_policy().unwrap();
        let mut verifier = Optimizer::new(settings);
        verifier.seed_policy(&policy).unwrap();
        assert_eq!(verifier.bounds(&root).1.unwrap().total, exported.total);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[20] ^= 1;
        std::fs::write(&path, bytes).unwrap();
        assert!(resumed.load_checkpoint(&path).is_err());
        assert!(resumed.is_proven(&root));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn four_slot_partitions_match_oracle_including_duplicate_colors() {
        let s = Settings::default();
        let space = enumerate_space(&s);
        let mut solver = Optimizer::new(s);
        let mut oracle = HashMap::new();
        for c in [
            vec![0, 1, 6, 7, 36, 37, 216, 217],
            vec![51, 97, 189, 300, 532, 985, 1295],
        ] {
            let expected = brute(&space, &c, &mut oracle);
            assert_eq!(solver.solve(&c, 1000).unwrap().unwrap().total, expected);
        }
    }

    #[test]
    fn strict_bounds_are_sound_without_any_exact_cache_entries() {
        let s = Settings {
            colors: 3,
            slots: 2,
            repeats: true,
        };
        let space = enumerate_space(&s);
        let mut oracle = HashMap::new();
        for mask in 1usize..(1 << space.len()) {
            let c: Vec<_> = (0..space.len())
                .filter(|i| mask & (1 << i) != 0)
                .map(|i| i as u16)
                .collect();
            let optimum = brute(&space, &c, &mut oracle);
            let mut solver = Optimizer::new(s);
            assert!(solver.solve(&c, optimum).unwrap().is_none());
            assert!(!solver.is_proven(&c) || solver.bounds(&c).1.unwrap().total == optimum);
            assert_eq!(
                solver.solve(&c, optimum + 1).unwrap().unwrap().total,
                optimum
            );
        }
    }
}
