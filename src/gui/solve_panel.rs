//! 整卷求解结果区(§5.1):[求解] 按钮 + 三分支展示。

use eframe::egui;

use crate::SolveOutcome;
use crate::gui::{palette, recommendation_row, Assets, SessionState};

pub fn show(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &SessionState,
    outcome: &mut Option<SolveOutcome>,
    suspects: &[usize],
) {
    ui.heading("整卷求解");
    if ui.button("求解").clicked() {
        *outcome = Some(crate::solve(&session.settings, &session.records));
    }
    let Some(oc) = outcome.as_ref() else {
        ui.label("录入全部记录后点击「求解」");
        return;
    };
    match oc {
        SolveOutcome::Unique(ans) => {
            ui.label("唯一答案:");
            ui.horizontal(|ui| {
                for &g in ans {
                    palette::big_gem(ui, assets, g);
                }
            });
        }
        SolveOutcome::Contradiction => {
            ui.colored_label(egui::Color32::RED, "记录矛盾!请检查录入,可逐条禁用定位。");
            if !suspects.is_empty() {
                let list =
                    suspects.iter().map(|i| (i + 1).to_string()).collect::<Vec<_>>().join("、");
                ui.label(format!("嫌疑记录:第 {list} 条(禁用后候选恢复非空)"));
            }
        }
        SolveOutcome::Ambiguous { candidates, recommendation } => {
            ui.label(format!("共 {} 个候选:", candidates.len()));
            if candidates.len() > 50 {
                ui.label("(超过 50 个,折叠为计数;继续录入记录或按推荐消歧)");
            } else {
                egui::ScrollArea::vertical()
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
