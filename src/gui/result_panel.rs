use std::time::Duration;

use eframe::egui;

use crate::gui::analysis::{AnalysisController, AnalysisPhase};
use crate::gui::{Assets, MAX_ROUNDS, SessionState, palette, suspects_row};
use crate::{Bound, Recommendation};

fn format_elapsed(elapsed: Duration) -> String {
    format!("{:.1} 秒", elapsed.as_secs_f64())
}

fn guarantee_fits(steps: usize, completed_rounds: usize) -> bool {
    steps <= MAX_ROUNDS.saturating_sub(completed_rounds) + 1
}

fn phase_status(phase: AnalysisPhase, has_verified_steps: bool, unique: bool) -> &'static str {
    match phase {
        AnalysisPhase::Idle => "等待分析",
        AnalysisPhase::Filtering => "正在筛选候选",
        AnalysisPhase::Recommending => "正在生成快速建议",
        AnalysisPhase::Refining => "快速建议 · 正在验证步数",
        AnalysisPhase::Complete if has_verified_steps => "步数已验证",
        AnalysisPhase::Complete if unique => "候选已核对",
        AnalysisPhase::Complete => "分析完成",
        AnalysisPhase::Failed => "分析失败",
    }
}

fn analysis_status(ui: &mut egui::Ui, analysis: &AnalysisController) {
    let has_verified_steps = matches!(
        analysis.cached.recommendation,
        Some(Recommendation::Guess {
            bound: Bound::GuaranteedSteps(_),
            ..
        })
    );
    let status = phase_status(
        analysis.phase,
        has_verified_steps,
        analysis.cached.ready && analysis.cached.candidates.len() == 1,
    );
    ui.horizontal(|ui| {
        if analysis.is_running() {
            ui.spinner();
        }
        ui.strong(status);
        if analysis.is_running() {
            ui.label(
                egui::RichText::new(format!("已用 {}", format_elapsed(analysis.elapsed()))).weak(),
            );
        }
    });
}

fn gems(ui: &mut egui::Ui, assets: &Assets, values: &[u8]) {
    ui.horizontal(|ui| {
        for &gem in values {
            palette::big_gem(ui, assets, gem);
        }
    });
}

fn highlighted_result(ui: &mut egui::Ui, assets: &Assets, title: &str, values: &[u8]) {
    ui.label(egui::RichText::new(title).strong().color(egui::Color32::RED));
    gems(ui, assets, values);
}

fn recommendation(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &SessionState,
    rec: &Recommendation,
    evidence: Option<&crate::core::strategy::RecommendationEvidence>,
    verified: bool,
) {
    let (guess, bound) = match rec {
        Recommendation::Answer(answer) => {
            highlighted_result(ui, assets, "唯一答案", answer);
            return;
        }
        Recommendation::Guess { guess, bound } => (guess, bound),
    };
    highlighted_result(ui, assets, "推荐下一猜", guess);

    if let Some(evidence) = evidence {
        ui.label(if evidence.is_possible_answer {
            "这是一个可能答案：猜中即可结束，也能继续缩小范围。"
        } else {
            "这是诊断猜测：它不在当前候选中，用来更有效地区分可能答案。"
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "按所有候选等可能计算，反馈后平均剩余 {:.1} 个；最坏情况剩 {} 个。",
                evidence.expected_remaining, evidence.worst_bucket
            ));
            ui.label(egui::RichText::new("计算说明").underline()).on_hover_text(format!(
                "信息熵 {:.2} 比特。平均值按当前 {} 个候选等概率计算。",
                evidence.entropy_bits, evidence.candidate_count
            ));
        });
        if evidence.sampled_search {
            ui.label(
                egui::RichText::new(
                    "为加快选择，建议猜测来自采样搜索；上面的平均值和最坏值已用全部候选重新计算。",
                )
                .weak(),
            );
        }
    }

    match bound {
        Bound::GuaranteedSteps(steps) if verified => {
            let remaining = MAX_ROUNDS.saturating_sub(session.records.len()) + 1;
            if guarantee_fits(*steps, session.records.len()) {
                ui.colored_label(
                    crate::gui::ACCENT,
                    format!("已验证：最多还需 {steps} 次提交，当前还可提交 {remaining} 次。"),
                );
            } else {
                ui.label(format!(
                    "已验证此策略最多还需 {steps} 次提交，但当前只剩 {remaining} 次机会。"
                ));
            }
            ui.label(
                egui::RichText::new(
                    "步数包含本次猜测和最终提交；保证依赖已录入反馈准确，并在后续各轮采用标有「步数已验证」的建议。",
                )
                .weak(),
            );
        }
        Bound::GuaranteedSteps(_) => {
            ui.label(egui::RichText::new("快速建议可先使用，步数保证仍在验证中。").weak());
        }
        Bound::Expected { .. } => {
            ui.label(egui::RichText::new("这是期望效果评估，不提供完成步数保证。").weak());
        }
    }
}

pub fn show(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &SessionState,
    analysis: &AnalysisController,
) {
    analysis_status(ui, analysis);
    ui.label(format!(
        "当前启用记录 {} / {} 条",
        analysis.cached.active_records, analysis.cached.total_records
    ));

    if let Some(error) = &analysis.error {
        ui.colored_label(egui::Color32::RED, format!("分析失败：{error}"));
        return;
    }
    if !analysis.cached.ready {
        ui.label(egui::RichText::new("结果会在分析完成后自动出现。").weak());
        return;
    }

    if analysis.cached.candidates.len() > 1 && analysis.cached.recommendation.is_some() {
        let description = match analysis.cached.policy_mode {
            Some(crate::core::policy::StrategyMode::Average) => "已证明平均最优：开局平均 4.340 次，最坏 6 次。",
            Some(crate::core::policy::StrategyMode::Worst) => "完整开局实测：平均 4.476 次，最坏 5 次。",
            None => "当前局面未覆盖预计算策略，采用实时分析。",
        };
        ui.label(egui::RichText::new(description).weak()).on_hover_text(
            "成绩按六色四槽、允许重复的 1296 个答案等可能、从开局持续采用同一策略且反馈准确计算，包含最终猜中答案的提交。平均策略已通过精确搜索证明总成本最小为 5625；最坏优先策略的成绩来自全量模拟。中途切换不沿用开局成绩；当前局面的保证见下方。"
        );
    }

    match analysis.cached.candidates.len() {
        0 => {
            ui.colored_label(egui::Color32::RED, "记录矛盾：没有符合全部启用记录的答案。");
            suspects_row(ui, &analysis.cached.suspects);
        }
        1 => {
            highlighted_result(ui, assets, "唯一答案", &analysis.cached.candidates[0]);
            ui.label(egui::RichText::new("仅当所有启用记录和反馈都准确时，此答案才成立。").weak());
        }
        _ if session.records.len() >= MAX_ROUNDS => {
            ui.label("猜测轮次已用完，请在上方提交最终答案。");
        }
        _ => {
            if let Some(rec) = &analysis.cached.recommendation {
                recommendation(
                    ui,
                    assets,
                    session,
                    rec,
                    analysis.cached.evidence.as_ref(),
                    analysis.phase == AnalysisPhase::Complete,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_text_keeps_short_waits_human_readable() {
        assert_eq!(
            format_elapsed(std::time::Duration::from_millis(420)),
            "0.4 秒"
        );
        assert_eq!(
            format_elapsed(std::time::Duration::from_millis(12_340)),
            "12.3 秒"
        );
    }

    #[test]
    fn guarantee_fits_only_when_all_required_submissions_remain() {
        assert!(guarantee_fits(3, 2));
        assert!(guarantee_fits(5, 2));
        assert!(!guarantee_fits(6, 2));
        assert!(guarantee_fits(1, 6));
    }

    #[test]
    fn completed_entropy_analysis_never_claims_step_verification() {
        assert_eq!(
            phase_status(AnalysisPhase::Complete, false, false),
            "分析完成"
        );
        assert_eq!(
            phase_status(AnalysisPhase::Complete, true, false),
            "步数已验证"
        );
        assert_eq!(
            phase_status(AnalysisPhase::Complete, false, true),
            "候选已核对"
        );
    }

}
