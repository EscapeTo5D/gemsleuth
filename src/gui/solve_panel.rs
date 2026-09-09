//! 整卷求解结果区:[求解] 按钮与结果说明,候选由外层右栏展示。

use eframe::egui;

use crate::gui::analysis::AnalysisController;
use crate::gui::{Assets, SessionState, primary_button, result_panel};

pub fn show(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &SessionState,
    requested: &mut bool,
    analysis: &AnalysisController,
) {
    ui.heading("整卷求解");
    if primary_button(ui, "求解", egui::vec2(110.0, 34.0)).clicked() {
        *requested = true;
    }
    if !*requested {
        ui.label(egui::RichText::new("录入全部记录后点击「求解」").weak());
        return;
    }
    result_panel::show(ui, assets, session, analysis);
}
