//! 算法耗时基准:`cargo run --release --example bench`

use gemsleuth::{filter_candidates, recommend, solve, suspect_records, Record, Settings};
use std::time::Instant;

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
    let t = Instant::now();
    let n = filter_candidates(&big, &ab).len();
    println!("filter 8×6 空间(262144)     : {:>10.1?}  ({} 候选)", t.elapsed(), n);

    let mut con = ab.clone();
    con.push(Record::new(vec![0, 1, 2, 3], 4, 0));
    let t = Instant::now();
    let _ = suspect_records(&s, &con);
    println!("suspect_records(3 条记录)   : {:>10.1?}", t.elapsed());
}
