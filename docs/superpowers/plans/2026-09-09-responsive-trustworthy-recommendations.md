# Recommendation responsiveness and evidence implementation plan

**Goal:** Reduce time to useful results and ensure every displayed conclusion belongs to the current records and accurately describes its guarantees.

**Architecture:** One cancellable background analysis shared by both tabs. Publish filtered candidates first, an entropy suggestion second, and a proven lookahead result last. Discard updates from superseded requests and wake egui when results arrive. Present candidate-wide metrics and explain limits in Chinese.

**Tech Stack:** Existing Rust, eframe/egui, rayon, standard channels and atomics; no new dependencies.

## Decisions and constraints

- Preserve the desktop app, fixed four slots, six colors, six feedback rounds and one final submission, current assets and two-column layout.
- Prefer staged analysis over cosmetic loading changes (which leave actual blocking) or a full algorithm rewrite (unnecessary scope).
- Record edits invalidate recommendations, suspects and answer validation immediately. No old result may be used as current evidence.
- Both tabs share analysis; the Solve button reveals the current analysis without synchronous recalculation. Editing requires clicking Solve again.
- Show real phase and elapsed time, never invented percentage or certainty. Worker errors must leave a retryable state.
- Quick recommendations are actionable suggestions with candidate-wide metrics but no invented step guarantee. Proven steps include this guess and final submission, assume accurate enabled records and continued recomputation, and must be compared with remaining opportunities.
- If search uses a sample, disclose it. Recalculate the chosen guess's displayed metrics over all candidates.
- Work in the current authorized workspace; no publishing or unrelated restructuring.

## Task 1: Core recommendation accuracy and cancellation

- [x] Add regression tests for sampled worst-bucket undercount and depth budget exhaustion; run them to establish failure.
- [x] Re-score a selected entropy guess over all candidates. Expose evidence: candidate count, whether guess is possible, expected remaining candidates, exact worst bucket and entropy, search sampling disclosure.
- [x] Expose fast entropy and cancellable recommendation entry points. Check cancellation inside lookahead loops and recursion; return no incomplete guarantee.
- [x] Preserve deterministic tie breaking and existing public recommendation behavior except corrected depth limits and truthful statistics.
- [x] Verify core tests and evidence against independent feedback partitions.

## Task 2: Staged background lifecycle

- [x] Add a focused `src/gui/analysis.rs` controller with generation-tagged updates, immediate cache invalidation, cancellation, phase/timing and worker-error handling.
- [x] Test old results rejected, candidates visible before recommendation, final update completion, cancellation on supersession and disconnected worker recovery.
- [x] Integrate controller into app initialization and logic; request repaint from worker plus periodic elapsed-time repaint.

## Task 3: Current and explained results

- [x] Share current analysis between assistant and Solve views; remove synchronous `solve()` from UI.
- [x] Invalidate Solve visibility on record mutation. Prevent final-answer validation and suspect labels from using stale candidates, including same-frame edits.
- [x] Keep left content constrained to left half and candidates on right. Show phase, elapsed time and active-record count.
- [x] Explain possible answer versus diagnostic guess, average/worst remaining candidates, sampling, conditional step guarantee and remaining-round budget. Unique means unique under entered enabled records.

## Verification

- [x] `cargo test --lib --release` (baseline: 44 passed).
- [x] Focused controller and evidence regressions, including real 24-candidate scenario and deterministic feedback partitions.
- [x] `cargo build --release` and `git diff --check`.
- [x] Extend release benchmark for candidates / quick recommendation / final recommendation timing. Report measured values as local measurements, not universal promises.
- [x] Independent code review for concurrency freshness and trust wording; resolve material findings.

## Progress

- Baseline tests: 44 passed; root cause inspection complete.
- Completed all three implementation tasks and independent review. Review fixes include immediate record counts, deferred suspect display, and a direct final-answer freshness guard.
- Final `cargo test --release --all-targets`: 69 passed, 0 failed; release executable built successfully; `git diff --check` clean (only Git line-ending notices).
- Headless egui regression covers a tall candidate list without pushing left-side content down. Native GUI has not been manually operated during this task.
- Local release benchmark, real two-record / 24-candidate scenario, five-run medians: candidates ready 89.1 µs; quick recommendation plus evidence ready 640.1 µs; final lookahead plus evidence ready 461.0 ms. These are computation times, not measured screen-presentation latency.
- Evidence regressions reproduce sampled worst bucket 438 versus full count 7,051 and a two-step result incorrectly accepted under depth cap one; both are fixed.
