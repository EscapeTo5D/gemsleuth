//! 陪玩助手结果区(§5.1):逐条增长记录,实时(无需按钮)显示候选与推荐。
//! 布局:左栏为状态与推荐,右栏为计算出的候选常驻区(单列滚动,高度撑满可用空间)。
//! 轮次用满(MAX_ROUNDS)后不再推荐下一猜,改为提示提交最终答案。

use eframe::egui;

use crate::gui::{
    candidates_block, palette, primary_button, recommendation_row, suspects_row, Assets,
    CachedAnalysis, MAX_ROUNDS, SessionState,
};
use crate::gui::records_panel::RecordEditor;

pub fn show(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &mut SessionState,
    editor: &mut RecordEditor,
    dirty: &mut bool,
    cached: &CachedAnalysis,
    computing: bool,
) {
    // 候选块顶部锚点:面板入口处(紧贴记录区与结果区之间的分割线,而非标题下方)
    let marks_top = ui.cursor().top();
    ui.horizontal(|ui| {
        ui.heading("陪玩助手");
        if computing {
            ui.colored_label(egui::Color32::RED, "(后台计算中,稍后自动刷新…)");
        }
    });
    match cached.candidates.len() {
        0 => {
            ui.colored_label(egui::Color32::RED, "记录矛盾!剩余候选为 0,请检查录入。");
            suspects_row(ui, &cached.suspects);
        }
        1 => {
            ui.strong("答案确定:");
            ui.horizontal(|ui| {
                for &g in &cached.candidates[0] {
                    palette::big_gem(ui, assets, g);
                }
            });
            // 一局终了:一键清空记录开新局(与录入区主操作同款琥珀主按钮)
            if primary_button(ui, "清除记录", egui::vec2(110.0, 30.0))
                .on_hover_text("清空全部记录,开始新的一局")
                .clicked()
            {
                session.records.clear();
                session.answer.clear();
                editor.reset();
                *dirty = true;
            }
        }
        n => {
            // 左栏(状态与推荐)走常规纵向流;候选块用显式矩形贴右。
            // 不用 allocate_ui 分栏:egui 0.36 它按内容宽度(而非 desired)推进光标,栏会挤在一起
            // 候选块占右半窗:左缘贴窗口中央分割线
            let right_w = ui.available_width() * 0.5;
            let rect = egui::Rect::from_min_max(
                egui::pos2(ui.max_rect().right() - right_w, marks_top),
                egui::pos2(ui.max_rect().right(), ui.max_rect().bottom()),
            );
            if session.records.len() >= MAX_ROUNDS {
                ui.label(egui::RichText::new("猜测轮次已用完:请在上方提交最终答案").weak());
            } else if let Some(rec) = &cached.recommendation {
                recommendation_row(ui, assets, rec);
            }
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
                |ui| {
                    candidates_block(
                        ui,
                        assets,
                        &cached.candidates,
                        &format!("剩余候选 {n} 个"),
                        "assistant_candidates",
                    );
                },
            );
        }
    }
}
