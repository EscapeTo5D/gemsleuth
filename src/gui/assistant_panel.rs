//! 陪玩助手结果区(§5.1):逐条增长记录,实时(无需按钮)显示候选与推荐。
//! 左栏的状态与推荐;历史记录及候选由外层工作区布局。
//! 轮次用满(MAX_ROUNDS)后不再推荐下一猜,改为提示提交最终答案。

use eframe::egui;

use crate::gui::analysis::AnalysisController;
use crate::gui::records_panel::RecordEditor;
use crate::gui::{Assets, SessionState, primary_button, result_panel};

pub fn show(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &mut SessionState,
    editor: &mut RecordEditor,
    dirty: &mut bool,
    analysis: &AnalysisController,
) {
    result_panel::show(ui, assets, session, analysis);
    if analysis.cached.ready && analysis.cached.candidates.len() == 1 {
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
}
