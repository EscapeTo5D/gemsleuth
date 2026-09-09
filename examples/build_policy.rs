//! Rebuild with: cargo run --release --example build_policy
use gemsleuth::{Settings, Record, Recommendation, enumerate_space, recommend, judge};
use std::collections::BTreeMap;

fn id(code: &[u8]) -> usize { code.iter().fold(0, |n, &v| n * 6 + v as usize) }

fn visit(candidates: Vec<Vec<u8>>, records: &mut Vec<Record>, rows: &mut Vec<String>, histogram: &mut [usize; 16]) -> usize {
    assert!(records.len() < 15);
    let guess = if candidates.len() == 1 { candidates[0].clone() } else {
        match recommend(&Settings::default(), records) {
            Recommendation::Answer(g) | Recommendation::Guess { guess: g, .. } => g,
        }
    };
    let mut buckets = BTreeMap::<_, Vec<Vec<u8>>>::new();
    for answer in &candidates {
        buckets.entry(judge(answer, &guess)).or_default().push(answer.clone());
    }
    let mut worst = 1;
    for ((exact, partial), bucket) in buckets {
        if exact == 4 { histogram[records.len() + 1] += bucket.len(); continue; }
        assert!(bucket.len() < candidates.len(), "policy makes no progress");
        records.push(Record::new(guess.clone(), exact, partial));
        worst = worst.max(1 + visit(bucket, records, rows, histogram));
        records.pop();
    }
    if candidates.len() > 1 {
        rows.push(format!("{};{};{}", candidates.iter().map(|c| id(c).to_string()).collect::<Vec<_>>().join(","), id(&guess), worst));
    }
    worst
}

fn main() {
    let started = std::time::Instant::now();
    let mut rows = Vec::new();
    let mut histogram = [0; 16];
    let worst = visit(enumerate_space(&Settings::default()), &mut Vec::new(), &mut rows, &mut histogram);
    assert_eq!(histogram.iter().sum::<usize>(), 1296);
    let total: usize = histogram.iter().enumerate().map(|(steps, count)| steps * count).sum();
    std::fs::write("assets/policy_6x4.txt", rows.join("\n") + "\n").unwrap();
    println!("answers=1296 nodes={} average={:.6} worst={} histogram={:?} elapsed={:?}", rows.len(), total as f64 / 1296.0, worst, histogram, started.elapsed());
}
