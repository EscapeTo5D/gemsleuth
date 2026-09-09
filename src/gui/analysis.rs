//! 当前记录的分阶段分析与后台任务生命周期。

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

use eframe::egui;

use super::MAX_ROUNDS;
use crate::core::strategy::{
    LOOKAHEAD_CANDIDATE_LIMIT, RecommendationEvidence, recommend_for_cancellable,
    recommend_quick_for_cancellable, recommendation_evidence,
};
use crate::{Recommendation, Record, Settings};
use crate::core::policy::StrategyMode;

#[derive(Clone, Default)]
pub struct CachedAnalysis {
    pub policy_mode: Option<StrategyMode>,
    /// false 代表尚未过滤新记录,不能把空 candidates 当成矛盾。
    pub ready: bool,
    pub candidates: Vec<Vec<u8>>,
    pub recommendation: Option<Recommendation>,
    pub suspects: Vec<usize>,
    pub evidence: Option<RecommendationEvidence>,
    pub active_records: usize,
    pub total_records: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisPhase {
    #[default]
    Idle,
    Filtering,
    Recommending,
    Refining,
    Complete,
    Failed,
}

struct Update {
    generation: u64,
    cached: CachedAnalysis,
    phase: AnalysisPhase,
}

#[derive(Default)]
pub struct AnalysisController {
    pub mode: StrategyMode,
    pub cached: CachedAnalysis,
    pub phase: AnalysisPhase,
    pub error: Option<String>,
    generation: u64,
    rx: Option<mpsc::Receiver<Update>>,
    cancelled: Option<Arc<AtomicBool>>,
    started: Option<Instant>,
    finished_elapsed: Option<Duration>,
}

impl AnalysisController {
    pub fn is_running(&self) -> bool {
        matches!(
            self.phase,
            AnalysisPhase::Filtering | AnalysisPhase::Recommending | AnalysisPhase::Refining
        )
    }

    pub fn elapsed(&self) -> Duration {
        self.finished_elapsed
            .unwrap_or_else(|| self.started.map_or(Duration::ZERO, |t| t.elapsed()))
    }

    fn begin(&mut self) -> (u64, Arc<AtomicBool>) {
        if let Some(cancelled) = self.cancelled.take() {
            cancelled.store(true, Ordering::Relaxed);
        }
        self.generation += 1;
        self.rx = None;
        self.cached = CachedAnalysis::default();
        self.phase = AnalysisPhase::Filtering;
        self.error = None;
        self.started = Some(Instant::now());
        self.finished_elapsed = None;
        let cancelled = Arc::new(AtomicBool::new(false));
        self.cancelled = Some(cancelled.clone());
        (self.generation, cancelled)
    }

    /// 只复制至多六条记录并启动线程;过滤和推荐均不占用界面线程。
    pub fn request(&mut self, settings: Settings, records: Vec<Record>, ctx: &egui::Context) {
        let (generation, cancelled) = self.begin();
        self.cached.active_records = records.iter().filter(|record| record.enabled).count();
        self.cached.total_records = records.len();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let wake = ctx.clone();
        let mode = self.mode;
        let spawned = std::thread::Builder::new()
            .name("analysis".into())
            .spawn(move || {
                // 即使算法意外 panic 也唤醒界面,由断开的 channel 转为可重试失败态。
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_analysis_with_mode(settings, records, &cancelled, mode, |cached, phase| {
                        let sent = tx
                            .send(Update {
                                generation,
                                cached,
                                phase,
                            })
                            .is_ok();
                        wake.request_repaint();
                        sent
                    });
                }));
                drop(tx);
                wake.request_repaint();
            });
        if spawned.is_err() {
            self.fail("无法启动分析，请重试");
        }
        ctx.request_repaint();
    }

    fn accept(&mut self, update: Update) {
        if update.generation != self.generation {
            return;
        }
        self.cached = update.cached;
        self.phase = update.phase;
        if !self.is_running() {
            self.finished_elapsed = Some(self.elapsed());
            self.cancelled = None;
        }
    }

    fn fail(&mut self, message: &str) {
        self.finished_elapsed = Some(self.elapsed());
        self.phase = AnalysisPhase::Failed;
        self.error = Some(message.into());
        self.rx = None;
        if let Some(cancelled) = self.cancelled.take() {
            cancelled.store(true, Ordering::Relaxed);
        }
    }

    pub fn poll(&mut self) {
        loop {
            let Some(rx) = &self.rx else { break };
            match rx.try_recv() {
                Ok(update) => self.accept(update),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.rx = None;
                    if self.is_running() {
                        self.fail("分析中断，请重试");
                    }
                    break;
                }
            }
        }
    }
}

impl Drop for AnalysisController {
    fn drop(&mut self) {
        if let Some(cancelled) = &self.cancelled {
            cancelled.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
fn run_analysis(
    settings: Settings,
    records: Vec<Record>,
    cancelled: &AtomicBool,
    publish: impl FnMut(CachedAnalysis, AnalysisPhase) -> bool,
) {
    run_analysis_with_mode(settings, records, cancelled, StrategyMode::Average, publish);
}

fn run_analysis_with_mode(
    settings: Settings,
    records: Vec<Record>,
    cancelled: &AtomicBool,
    mode: StrategyMode,
    mut publish: impl FnMut(CachedAnalysis, AnalysisPhase) -> bool,
) {
    if cancelled.load(Ordering::Relaxed) {
        return;
    }
    let candidates = crate::filter_candidates(&settings, &records);
    if cancelled.load(Ordering::Relaxed) {
        return;
    }
    let mut cached = CachedAnalysis {
        ready: true,
        candidates,
        active_records: records.iter().filter(|r| r.enabled).count(),
        total_records: records.len(),
        ..Default::default()
    };
    if cached.candidates.is_empty() {
        cached.suspects = crate::suspect_records(&settings, &records);
        if !cancelled.load(Ordering::Relaxed) {
            publish(cached, AnalysisPhase::Complete);
        }
        return;
    }
    if records.len() >= MAX_ROUNDS {
        publish(cached, AnalysisPhase::Complete);
        return;
    }
    if !publish(cached.clone(), AnalysisPhase::Recommending) || cancelled.load(Ordering::Relaxed) {
        return;
    }
    if let Some(recommendation) = crate::core::policy::lookup_mode(&settings, &cached.candidates, mode) {
        cached.policy_mode = Some(mode);
        cached.evidence = Some(recommendation_evidence(&settings, &cached.candidates, &recommendation));
        cached.recommendation = Some(recommendation);
        if !cancelled.load(Ordering::Relaxed) {
            publish(cached, AnalysisPhase::Complete);
        }
        return;
    }
    let Some(quick) = recommend_quick_for_cancellable(&settings, &cached.candidates, cancelled)
    else {
        return;
    };
    if cancelled.load(Ordering::Relaxed) {
        return;
    }
    cached.evidence = Some(recommendation_evidence(
        &settings,
        &cached.candidates,
        &quick,
    ));
    cached.recommendation = Some(quick);
    let refine = (3..=LOOKAHEAD_CANDIDATE_LIMIT).contains(&cached.candidates.len());
    let phase = if refine {
        AnalysisPhase::Refining
    } else {
        AnalysisPhase::Complete
    };
    if !publish(cached.clone(), phase) || !refine || cancelled.load(Ordering::Relaxed) {
        return;
    }
    if let Some(recommendation) =
        recommend_for_cancellable(&settings, &cached.candidates, cancelled)
    {
        cached.evidence = Some(recommendation_evidence(
            &settings,
            &cached.candidates,
            &recommendation,
        ));
        cached.recommendation = Some(recommendation);
        if !cancelled.load(Ordering::Relaxed) {
            publish(cached, AnalysisPhase::Complete);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(candidates: Vec<Vec<u8>>) -> CachedAnalysis {
        CachedAnalysis {
            ready: true,
            candidates,
            ..Default::default()
        }
    }

    #[test]
    fn superseding_request_cancels_worker_and_invalidates_evidence_immediately() {
        let mut analysis = AnalysisController::default();
        let (_, cancelled) = analysis.begin();
        analysis.cached = ready(vec![vec![0, 0, 0, 0]]);
        analysis.cached.suspects = vec![1];
        analysis.begin();
        assert!(cancelled.load(Ordering::Relaxed));
        assert!(!analysis.cached.ready);
        assert!(analysis.cached.candidates.is_empty());
        assert!(analysis.cached.suspects.is_empty());
    }

    #[test]
    fn late_result_cannot_replace_current_records_or_finish_current_job() {
        let mut analysis = AnalysisController::default();
        let (old, _) = analysis.begin();
        let (current, _) = analysis.begin();
        analysis.accept(Update {
            generation: old,
            cached: ready(vec![vec![0; 4]]),
            phase: AnalysisPhase::Complete,
        });
        assert!(!analysis.cached.ready);
        assert!(analysis.is_running());
        analysis.accept(Update {
            generation: current,
            cached: ready(vec![vec![1; 4]]),
            phase: AnalysisPhase::Complete,
        });
        assert_eq!(analysis.cached.candidates, vec![vec![1; 4]]);
        assert!(!analysis.is_running());
    }

    #[test]
    fn candidate_update_is_usable_before_recommendation_finishes() {
        let mut analysis = AnalysisController::default();
        let (generation, _) = analysis.begin();
        analysis.accept(Update {
            generation,
            cached: ready(vec![vec![1; 4], vec![2; 4]]),
            phase: AnalysisPhase::Recommending,
        });
        assert!(analysis.cached.ready);
        assert!(analysis.cached.recommendation.is_none());
        assert!(analysis.is_running());
    }

    #[test]
    fn request_displays_current_record_counts_before_worker_results_arrive() {
        let mut disabled = Record::new(vec![0; 4], 4, 0);
        disabled.enabled = false;
        let records = vec![Record::new(vec![1; 4], 4, 0), disabled];
        let mut analysis = AnalysisController::default();
        analysis.request(Settings::default(), records, &egui::Context::default());
        assert_eq!(analysis.cached.active_records, 1);
        assert_eq!(analysis.cached.total_records, 2);
        assert!(!analysis.cached.ready);
    }

    #[test]
    fn disconnected_worker_stops_waiting_and_can_retry() {
        let mut analysis = AnalysisController::default();
        analysis.begin();
        let (tx, rx) = mpsc::channel();
        analysis.rx = Some(rx);
        drop(tx);
        analysis.poll();
        assert_eq!(analysis.phase, AnalysisPhase::Failed);
        assert!(!analysis.is_running());
        assert!(analysis.error.is_some());
        analysis.begin();
        assert!(analysis.error.is_none());
        assert!(analysis.is_running());
    }

    #[test]
    fn completed_worker_disconnect_is_not_an_error() {
        let mut analysis = AnalysisController::default();
        let (generation, _) = analysis.begin();
        let (tx, rx) = mpsc::channel();
        analysis.rx = Some(rx);
        tx.send(Update {
            generation,
            cached: ready(vec![vec![1; 4]]),
            phase: AnalysisPhase::Complete,
        })
        .unwrap();
        drop(tx);
        analysis.poll();
        assert_eq!(analysis.phase, AnalysisPhase::Complete);
        assert!(analysis.error.is_none());
    }

    #[test]
    fn failed_refinement_keeps_current_candidates_until_retry_invalidates_them() {
        let mut analysis = AnalysisController::default();
        let (generation, _) = analysis.begin();
        let (tx, rx) = mpsc::channel();
        analysis.rx = Some(rx);
        tx.send(Update {
            generation,
            cached: ready(vec![vec![1; 4], vec![2; 4]]),
            phase: AnalysisPhase::Recommending,
        })
        .unwrap();
        drop(tx);
        analysis.poll();
        assert_eq!(analysis.phase, AnalysisPhase::Failed);
        assert!(analysis.cached.ready);
        assert_eq!(analysis.cached.candidates.len(), 2);
        analysis.begin();
        assert!(!analysis.cached.ready);
        assert!(analysis.cached.candidates.is_empty());
    }

    #[test]
    fn real_analysis_publishes_candidates_then_quick_suggestion_then_proof() {
        let settings = Settings::default();
        let records = vec![
            Record::new(vec![3, 1, 2, 0], 1, 0),
            Record::new(vec![3, 1, 2, 5], 1, 0),
        ];
        let mut updates = Vec::new();
        run_analysis(
            settings,
            records,
            &AtomicBool::new(false),
            |cached, phase| {
                updates.push((cached, phase));
                true
            },
        );
        assert_eq!(updates.len(), 3);
        assert_eq!(updates[0].0.candidates.len(), 24);
        assert!(updates[0].0.recommendation.is_none());
        assert_eq!(updates[0].1, AnalysisPhase::Recommending);
        assert!(matches!(
            updates[1].0.recommendation,
            Some(Recommendation::Guess {
                bound: crate::Bound::Expected { .. },
                ..
            })
        ));
        assert_eq!(updates[1].1, AnalysisPhase::Refining);
        assert!(matches!(
            updates[2].0.recommendation,
            Some(Recommendation::Guess {
                bound: crate::Bound::GuaranteedSteps(_),
                ..
            })
        ));
        assert_eq!(updates[2].1, AnalysisPhase::Complete);
    }

    #[test]
    fn exhausted_rounds_publish_candidates_without_wasting_time_on_a_guess() {
        let records = vec![Record::new(vec![0, 1, 2, 3], 0, 0); MAX_ROUNDS];
        let mut updates = Vec::new();
        run_analysis(
            Settings::default(),
            records,
            &AtomicBool::new(false),
            |cached, phase| {
                updates.push((cached, phase));
                true
            },
        );
        assert_eq!(updates.len(), 1);
        assert!(updates[0].0.ready);
        assert!(updates[0].0.recommendation.is_none());
        assert_eq!(updates[0].1, AnalysisPhase::Complete);
    }

    #[test]
    fn cancellation_after_filtering_never_publishes_a_recommendation() {
        let cancelled = AtomicBool::new(false);
        let mut count = 0;
        run_analysis(Settings::default(), vec![], &cancelled, |cached, _| {
            count += 1;
            assert!(cached.recommendation.is_none());
            cancelled.store(true, Ordering::Relaxed);
            true
        });
        assert_eq!(count, 1);
    }
}
