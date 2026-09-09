//! 算法耗时基准:`cargo run --release --example bench`

use gemsleuth::{filter_candidates, recommend, recommend_quick_for, recommendation_evidence, solve, suspect_records, Record, Settings};
use std::time::{Duration, Instant};

fn main() {
    let s = Settings::default();

    let t = Instant::now();
    let full = filter_candidates(&s, &[]);
    println!("filter(空记录, 1296 空间)    : {:>10.1?}  ({} 候选)", t.elapsed(), full.len());

    let t = Instant::now();
    let _ = recommend(&s, &[]);
    println!("熵推荐 1296 候选×1296 猜测   : {:>10.1?}", t.elapsed());

    let ab = vec![
        Record::new(vec![3, 1, 2, 0], 1, 0),
        Record::new(vec![3, 1, 2, 5], 1, 0),
    ];
    let t = Instant::now();
    let _ = recommend(&s, &ab);
    println!("前瞻推荐 24 候选(1296 猜测) : {:>10.1?}", t.elapsed());

    let t = Instant::now();
    let _ = solve(&s, &ab);
    println!("solve(过滤+前瞻推荐)        : {:>10.1?}", t.elapsed());

    let big = Settings { colors: 8, slots: 6, repeats: true };
    let six_slot_records = [Record::new(vec![3, 1, 2, 0, 4, 5], 1, 0)];
    let t = Instant::now();
    let n = filter_candidates(&big, &six_slot_records).len();
    println!("filter 8×6 空间(262144)     : {:>10.1?}  ({} 候选)", t.elapsed(), n);

    let mut con = ab.clone();
    con.push(Record::new(vec![0, 1, 2, 3], 4, 0));
    let t = Instant::now();
    let _ = suspect_records(&s, &con);
    println!("suspect_records(3 条记录)   : {:>10.1?}", t.elapsed());

    // 接近界面的阶段顺序:每项是自录入起的累计等待时间。
    // recommend 公共入口会再过滤一次,此处包含这部分开销。
    let mut filtered = Vec::new();
    let mut quick = Vec::new();
    let mut complete = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        let candidates = filter_candidates(&s, &ab);
        filtered.push(t.elapsed());
        let rec = recommend_quick_for(&s, &candidates);
        std::hint::black_box(recommendation_evidence(&s, &candidates, &rec));
        quick.push(t.elapsed());
        let final_rec = recommend(&s, &ab);
        std::hint::black_box(recommendation_evidence(&s, &candidates, &final_rec));
        complete.push(t.elapsed());
    }
    fn median(samples: &mut [Duration]) -> Duration {
        samples.sort_unstable();
        samples[samples.len() / 2]
    }
    println!("24 候选分阶段累计等待(5 次中位数):");
    println!("  候选就绪                : {:>10.1?}", median(&mut filtered));
    println!("  快速建议及依据就绪      : {:>10.1?}", median(&mut quick));
    println!("  前瞻验证及依据就绪      : {:>10.1?}", median(&mut complete));
}
