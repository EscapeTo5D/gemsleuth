//! Exact offline optimization: --seconds 60 --out target/exact-search [--fresh].
use gemsleuth::{Settings, core::optimal::Optimizer};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut seconds = 60u64;
    let mut out = PathBuf::from("target/exact-search");
    let mut fresh = false;
    let mut seed = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seconds" => seconds = args.next().ok_or("missing seconds")?.parse()?,
            "--out" => out = args.next().ok_or("missing output directory")?.into(),
            "--fresh" => fresh = true,
            "--seed" => seed = Some(PathBuf::from(args.next().ok_or("missing seed policy")?)),
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    std::fs::create_dir_all(&out)?;
    let checkpoint = out.join("search.checkpoint");
    let mut solver = Optimizer::new(Settings::default());
    if checkpoint.exists() && !fresh {
        solver.load_checkpoint(&checkpoint)?;
        println!("resumed {} states", solver.entries());
    }
    solver.seed_policy(include_str!("../assets/policy_average.txt"))?;
    if let Some(path) = seed {
        solver.seed_policy(&std::fs::read_to_string(path)?)?;
    }
    let root = solver.root();
    let start = Instant::now();
    solver.deadline = Some(start + Duration::from_secs(seconds));
    let refined = solver.refine_incumbent().is_ok();
    let (refined_policy, _) = solver.export_policy()?;
    solver.seed_policy(&refined_policy)?;
    let cap = solver.bounds(&root).1.unwrap().total + 1;
    let finished = refined && solver.solve(&root, cap).is_ok();
    let (policy, result) = solver.export_policy()?;
    // Rebuild from the exported guesses, independently of cached costs and lower bounds.
    let mut verification = Optimizer::new(Settings::default());
    verification.seed_policy(&policy)?;
    let verified = verification.bounds(&root).1.unwrap();
    assert_eq!(
        (result.total, result.worst),
        (verified.total, verified.worst)
    );
    assert_eq!(
        gemsleuth::core::policy::verify_policy(&policy)?,
        (result.total, result.worst)
    );
    solver.seed_policy(&policy)?;
    let (lower, _) = solver.bounds(&root);
    assert!(lower <= result.total);
    let proven = solver.is_proven(&root);
    solver.save_checkpoint(&checkpoint)?;
    std::fs::write(out.join("best-policy.txt"), &policy)?;
    let report = format!(
        "{{\n  \"answers\": 1296,\n  \"lower_total\": {lower},\n  \"upper_total\": {},\n  \"average\": {:.9},\n  \"worst\": {},\n  \"proven_optimal\": {proven},\n  \"finished_search\": {finished},\n  \"elapsed_seconds\": {:.3},\n  \"states\": {}\n}}\n",
        result.total,
        result.total as f64 / 1296.0,
        result.worst,
        start.elapsed().as_secs_f64(),
        solver.entries()
    );
    std::fs::write(out.join("report.json"), &report)?;
    println!("{report}");
    Ok(())
}
