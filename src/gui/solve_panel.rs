//! 整卷求解结果区(§5.1):[求解] 按钮 + 三分支展示。

use eframe::egui;

use crate::SolveOutcome;
use crate::gui::{palette, primary_button, recommendation_row, suspects_row, Assets, SessionState};

pub fn show(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &SessionState,
    outcome: &mut Option<SolveOutcome>,
    suspects: &[usize],
) {
    ui.heading("整卷求解");
    if primary_button(ui, "求解", egui::vec2(110.0, 34.0)).clicked() {
        *outcome = Some(crate::solve(&session.settings, &session.records));
    }
    let Some(oc) = outcome.as_ref() else {
        ui.label(egui::RichText::new("录入全部记录后点击「求解」").weak());
        return;
    };
    match oc {
        SolveOutcome::Unique(ans) => {
            ui.strong("唯一答案:");
            ui.horizontal(|ui| {
                for &g in ans {
                    palette::big_gem(ui, assets, g);
                }
            });
        }
        SolveOutcome::Contradiction => {
            ui.colored_label(egui::Color32::RED, "记录矛盾!请检查录入,可逐条禁用定位。");
            suspects_row(ui, suspects);
        }
        SolveOutcome::Ambiguous { candidates, recommendation } => {
            ui.strong(format!("共 {} 个候选:", candidates.len()));
            if candidates.len() > 50 {
                ui.label(egui::RichText::new("(超过 50 个,折叠为计数;继续录入记录或按推荐消歧)").weak());
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("solve_candidates")
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for c in candidates {
                            ui.horizontal(|ui| {
                                for &g in c {
                                    palette::small_gem(ui, assets, g);
                                }
                            });
                        }
                    });
            }
            recommendation_row(ui, assets, recommendation);
        }
    }
}
