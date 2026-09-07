//! egui 薄壳:会话状态、脏标记重算、设置栏、Tab 切换(§5)。

use eframe::egui;

use crate::{Record, Recommendation, Settings, SolveOutcome};

use records_panel::RecordEditor;

pub mod assistant_panel;
pub mod palette;
pub mod records_panel;
pub mod solve_panel;
pub mod assets;

pub use assets::Assets;

#[derive(Default)]
pub struct SessionState {
    pub settings: Settings,
    pub records: Vec<Record>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tab { Solve, Assistant }

#[derive(Default)]
pub struct CachedAnalysis {
    pub candidates: Vec<Vec<u8>>,
    pub recommendation: Option<Recommendation>, // 候选空时为 None
    pub suspects: Vec<usize>,                   // 矛盾时的嫌疑记录下标
}

pub struct GemsleuthApp {
    pub session: SessionState,
    pub tab: Tab,
    pub pending_settings: Option<Settings>, // 待确认的新设置(确认弹窗)
    pub dirty: bool,                        // 任一会话变更 → 重算
    pub cached: CachedAnalysis,
    pub solve_outcome: Option<SolveOutcome>, // 整卷求解快照(点击求解时更新)
    pub font_warning: bool,
    pub assets: Assets,
    pub editor: RecordEditor,
}

impl GemsleuthApp {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        let font_warning = !install_cjk_fonts(&cc.egui_ctx);
        Self {
            session: SessionState::default(),
            tab: Tab::Solve,
            pending_settings: None,
            dirty: true,
            cached: CachedAnalysis::default(),
            solve_outcome: None,
            font_warning,
            assets: Assets::load(&cc.egui_ctx),
            editor: RecordEditor::default(),
        }
    }
}

/// 运行时探测系统中文字体(不分发字体文件,规避许可,§5.3)。
fn install_cjk_fonts(ctx: &egui::Context) -> bool {
    const CANDIDATES: &[&str] = &[
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
    ];
    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("cjk".into(), egui::FontData::from_owned(bytes).into());
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.get_mut(&fam).unwrap().insert(0, "cjk".into());
            }
            ctx.set_fonts(fonts);
            return true;
        }
    }
    false
}

fn settings_bar(ui: &mut egui::Ui, session: &mut SessionState, pending: &mut Option<Settings>) {
    ui.horizontal(|ui| {
        ui.label("设置:");
        let mut next = session.settings;
        egui::ComboBox::from_label("颜色数")
            .selected_text(format!("{}", session.settings.colors))
            .show_ui(ui, |ui| {
                for c in 4..=8 {
                    ui.selectable_value(&mut next.colors, c, format!("{c}"));
                }
            });
        // 不允许重复时,槽位数选项收窄到 ≤ 颜色数(§4.5 禁止非法组合)
        egui::ComboBox::from_label("槽位数")
            .selected_text(format!("{}", session.settings.slots))
            .show_ui(ui, |ui| {
                let max = if next.repeats { 6 } else { next.colors.min(6) };
                for s in 3..=max {
                    ui.selectable_value(&mut next.slots, s, format!("{s}"));
                }
            });
        ui.checkbox(&mut next.repeats, "允许重复");
        if !next.repeats && next.slots > next.colors {
            ui.colored_label(egui::Color32::RED, "不允许重复时槽位数不能超过颜色数");
        } else if next != session.settings {
            *pending = Some(next); // 任何设置变化都弹确认(确认后清空记录,§4.5)
        }
        if ui.button("重置会话").clicked() {
            *pending = Some(session.settings); // 设置不变,确认后仅清空记录
        }
    });
}

fn confirm_dialog(
    ui: &mut egui::Ui,
    pending: &mut Option<Settings>,
    session: &mut SessionState,
    editor: &mut RecordEditor,
    dirty: &mut bool,
) {
    if pending.is_none() {
        return;
    }
    let ctx = ui.ctx().clone();
    egui::Window::new("确认修改设置")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(&ctx, |ui| {
            ui.label("修改设置将清空当前全部记录,确定吗?");
            ui.horizontal(|ui| {
                if ui.button("确定").clicked() {
                    session.settings = pending.take().unwrap();
                    session.records.clear();
                    editor.reset(); // 同步清空编辑器,防止悬空 editing 索引(规格 §5.1)
                    *dirty = true;
                }
                if ui.button("取消").clicked() {
                    *pending = None;
                }
            });
        });
}

impl eframe::App for GemsleuthApp {
    /// 每帧 UI 前调用,禁止画 UI——正好承载脏标记重算(§5.1 实时刷新且不卡帧)。
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.dirty {
            return;
        }
        let candidates = crate::filter_candidates(&self.session.settings, &self.session.records);
        let (recommendation, suspects) = if candidates.is_empty() {
            (
                None,
                crate::suspect_records(&self.session.settings, &self.session.records),
            )
        } else {
            (
                Some(crate::core::strategy::recommend_for(&self.session.settings, &candidates)),
                vec![],
            )
        };
        self.cached = CachedAnalysis { candidates, recommendation, suspects };
        self.dirty = false;
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            if self.font_warning {
                ui.colored_label(egui::Color32::YELLOW, "警告:未找到系统中文字体,中文可能无法显示");
            }
            egui::Panel::top(egui::Id::new("settings")).show(ui, |ui| {
                settings_bar(ui, &mut self.session, &mut self.pending_settings);
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Solve, "整卷求解");
                ui.selectable_value(&mut self.tab, Tab::Assistant, "陪玩助手");
            });
            ui.separator();
            records_panel::show(
                ui,
                &self.assets,
                &mut self.session,
                &mut self.editor,
                &mut self.dirty,
                &self.cached.suspects,
            );
            ui.separator();
            // Task 13/14 接入结果区,暂以占位标签过渡
            ui.label(format!("实时候选数:{}", self.cached.candidates.len()));
            confirm_dialog(
                ui,
                &mut self.pending_settings,
                &mut self.session,
                &mut self.editor,
                &mut self.dirty,
            );
        });
    }
}
