//! 整卷求解结果区(§5.1):[求解] 按钮 + 三分支展示。候选分支与陪玩助手同款右栏常驻区。

use eframe::egui;

use crate::SolveOutcome;
use crate::gui::{
    candidates_block, palette, primary_button, recommendation_row, suspects_row, Assets, MAX_ROUNDS,
    SessionState,
};

pub fn show(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &SessionState,
    outcome: &mut Option<SolveOutcome>,
    suspects: &[usize],
) {
    // 候选块顶部锚点:面板入口处(紧贴记录区与结果区之间的分割线,而非标题下方)
    let marks_top = ui.cursor().top();
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
            // 左栏(推荐)走常规纵向流;候选块用显式矩形贴右(理由同 assistant_panel)
            // 候选块占右半窗:左缘贴窗口中央分割线
            let right_w = ui.available_width() * 0.5;
            let rect = egui::Rect::from_min_max(
                egui::pos2(ui.max_rect().right() - right_w, marks_top),
                egui::pos2(ui.max_rect().right(), ui.max_rect().bottom()),
            );
            if session.records.len() >= MAX_ROUNDS {
                ui.label(egui::RichText::new("猜测轮次已用完:请在上方提交最终答案").weak());
            } else {
                recommendation_row(ui, assets, recommendation);
            }
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
                |ui| {
                    candidates_block(
                        ui,
                        assets,
                        candidates,
                        &format!("共 {} 个候选:", candidates.len()),
                        "solve_candidates",
                    );
                },
            );
        }
    }
}
