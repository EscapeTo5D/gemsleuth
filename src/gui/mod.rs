//! egui 薄壳:会话状态、脏标记重算、设置栏、Tab 切换(§5)。

use eframe::egui;

use crate::{Bound, Record, Recommendation, Settings, SolveOutcome};

use records_panel::RecordEditor;

pub mod assistant_panel;
pub mod palette;
pub mod records_panel;
pub mod solve_panel;
pub mod assets;

pub use assets::Assets;

#[derive(Clone, Copy, PartialEq)]
pub enum Tab { Assistant, Solve }

/// 游戏规则:最多 6 轮猜测,之后必须提交最终答案。
pub const MAX_ROUNDS: usize = 6;

/// 槽位数固定为 4(对齐当前真实游戏);核心引擎仍支持 3..=6,仅 GUI 固定。
pub const SLOTS: usize = 4;

/// 会话默认:颜色数固定为 6(palette::COLORS),槽位数固定为 4(SLOTS)。
pub struct SessionState {
    pub settings: Settings,
    pub records: Vec<Record>,
    pub answer: Vec<u8>, // 第 MAX_ROUNDS+1 轮的最终答案草稿
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            settings: Settings { colors: palette::COLORS, slots: SLOTS, ..Default::default() },
            records: Vec::new(),
            answer: Vec::new(),
        }
    }
}

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
        customize_visuals(&cc.egui_ctx);
        Self {
            session: SessionState::default(),
            tab: Tab::Assistant,
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

/// 主题强调色(琥珀金):主按钮/选中 Tab/链接。
pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(235, 172, 58);

/// 主操作按钮(琥珀底黑字,视觉上高于普通按钮)。
pub fn primary_button(ui: &mut egui::Ui, label: &str, min_size: egui::Vec2) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).strong().color(egui::Color32::BLACK))
            .fill(ACCENT)
            .min_size(min_size),
    )
}

/// 段式 Tab 按钮:选中琥珀底黑字,未选中暗底前景色。
fn tab_button(ui: &mut egui::Ui, selected: bool, label: &str) -> bool {
    let (fill, text) = if selected {
        (ACCENT, egui::Color32::BLACK)
    } else {
        (
            egui::Color32::from_gray(40),
            ui.style().visuals.widgets.noninteractive.fg_stroke.color,
        )
    };
    ui.add(
        egui::Button::new(egui::RichText::new(label).strong().size(15.0).color(text))
            .fill(fill)
            .min_size(egui::vec2(128.0, 32.0)),
    )
    .clicked()
}

/// 全局主题:深色底 + 琥珀点缀 + 圆角,突出彩色宝石图标。
fn customize_visuals(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.visuals = egui::Visuals::dark();
        let v = &mut style.visuals;
        v.window_corner_radius = egui::CornerRadius::same(8);
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
            w.corner_radius = egui::CornerRadius::same(6);
        }
        v.widgets.inactive.weak_bg_fill = egui::Color32::from_gray(40);
        v.widgets.hovered.weak_bg_fill = egui::Color32::from_gray(56);
        v.widgets.active.weak_bg_fill = egui::Color32::from_gray(64);
        v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(78));
        v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(105));
        v.selection.bg_fill = egui::Color32::from_rgb(122, 88, 28); // 深琥珀,保证白字可读
        v.selection.stroke = egui::Stroke::new(1.0, ACCENT);
        v.hyperlink_color = ACCENT;
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
    });
}

/// 推荐猜测展示行(两结果面板共用,§5.1)。标签独立成行,宝石与备注另起一行。
pub fn recommendation_row(ui: &mut egui::Ui, assets: &Assets, rec: &Recommendation) {
    ui.strong("推荐下一猜:");
    ui.horizontal(|ui| {
        match rec {
            Recommendation::Answer(ans) => {
                for &g in ans {
                    palette::big_gem(ui, assets, g);
                }
            }
            Recommendation::Guess { guess, bound } => {
                for &g in guess {
                    palette::big_gem(ui, assets, g);
                }
                match bound {
                    Bound::GuaranteedSteps(n) => {
                        ui.label(egui::RichText::new(format!("(精确前瞻:最多还需 {n} 步)")).weak());
                    }
                    Bound::Expected { entropy_bits, worst_bucket } => {
                        ui.label(egui::RichText::new(format!(
                            "(熵推荐:期望信息量 {entropy_bits:.2} 比特,最坏情况剩 {worst_bucket} 个)"
                        )).weak());
                    }
                }
            }
        }
    });
}

/// 矛盾时的嫌疑记录提示行(两结果面板共用)。
pub fn suspects_row(ui: &mut egui::Ui, suspects: &[usize]) {
    if !suspects.is_empty() {
        let list = suspects.iter().map(|i| (i + 1).to_string()).collect::<Vec<_>>().join("、");
        ui.label(format!("嫌疑记录:第 {list} 条(禁用后候选恢复非空)"));
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

/// 背景图:画进根 Ui 所在的 Background 层(LayerId::background()),铺满整个窗口
/// (cover:保持宽高比、居中、超出裁剪),再压一层半透明黑保证前景可读。
/// 本函数在所有面板之前调用;同一层内按绘制顺序叠加,故背景始终垫底。
/// 注意不能用自建 Order::Background 图层——同档图层按 Id 哈希排序,可能盖住面板内容。
fn paint_background(ui: &mut egui::Ui, assets: &Assets) {
    // 首帧输入尚未带屏幕矩形,跳过一帧不画
    let Some(screen) = ui.input(|i| i.raw.screen_rect) else { return };
    let size = assets.bg.size_vec2();
    let scale = (screen.width() / size.x).max(screen.height() / size.y);
    let dst = egui::Rect::from_center_size(screen.center(), size * scale);
    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    let painter = ui.ctx().layer_painter(egui::LayerId::background());
    painter.image(assets.bg.id(), dst, uv, egui::Color32::WHITE);
    painter.rect_filled(screen, 0.0, egui::Color32::from_black_alpha(90));
}

fn settings_bar(ui: &mut egui::Ui, session: &mut SessionState, pending: &mut Option<Settings>) {
    ui.horizontal(|ui| {
        ui.strong("设置:");
        let mut next = session.settings;
        // 颜色数固定为 6(palette::COLORS),槽位数固定为 4(SLOTS),仅保留重复开关
        ui.checkbox(&mut next.repeats, "允许重复");
        if next != session.settings {
            *pending = Some(next); // 任何设置变化都弹确认(确认后清空记录,§4.5)
        }
        // 重置会话靠右
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("重置会话").clicked() {
                *pending = Some(session.settings); // 设置不变,确认后仅清空记录
            }
        });
    });
}

fn confirm_dialog(
    ui: &mut egui::Ui,
    pending: &mut Option<Settings>,
    session: &mut SessionState,
    editor: &mut RecordEditor,
    dirty: &mut bool,
    solve_outcome: &mut Option<SolveOutcome>,
) {
    if pending.is_none() {
        return;
    }
    // pending 与当前设置相同 ⇒ 来自「重置会话」,文案只说清空记录
    let reset_only = pending.as_ref() == Some(&session.settings);
    let (title, body) = if reset_only {
        ("重置会话", "确定要清空当前全部记录吗?")
    } else {
        ("确认修改设置", "修改重复设置将清空当前全部记录,确定吗?")
    };
    let ctx = ui.ctx().clone();
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(&ctx, |ui| {
            ui.label(body);
            ui.horizontal(|ui| {
                if primary_button(ui, "确定", egui::vec2(84.0, 30.0)).clicked() {
                    session.settings = pending.take().unwrap();
                    session.records.clear();
                    session.answer.clear();
                    editor.reset(); // 同步清空编辑器,防止悬空 editing 索引(规格 §5.1)
                    *solve_outcome = None; // 记录全清,求解快照一并失效
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
        paint_background(ui, &self.assets);
        // 面板填充透明,否则会盖住背景层
        let frame = egui::Frame::central_panel(ui.style())
            .inner_margin(egui::Margin::same(12))
            .fill(egui::Color32::TRANSPARENT);
        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            if self.font_warning {
                ui.colored_label(egui::Color32::YELLOW, "警告:未找到系统中文字体,中文可能无法显示");
            }
            egui::Panel::top(egui::Id::new("settings"))
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    settings_bar(ui, &mut self.session, &mut self.pending_settings);
                });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if tab_button(ui, self.tab == Tab::Assistant, "陪玩助手") {
                    self.tab = Tab::Assistant;
                }
                if tab_button(ui, self.tab == Tab::Solve, "整卷求解") {
                    self.tab = Tab::Solve;
                }
            });
            ui.add_space(4.0);
            records_panel::show(
                ui,
                &self.assets,
                &mut self.session,
                &mut self.editor,
                &self.cached,
                &mut self.dirty,
            );
            ui.separator();
            match self.tab {
                Tab::Assistant => {
                    assistant_panel::show(ui, &self.assets, &self.session, &self.cached)
                }
                Tab::Solve => solve_panel::show(
                    ui,
                    &self.assets,
                    &self.session,
                    &mut self.solve_outcome,
                    &self.cached.suspects,
                ),
            }
            confirm_dialog(
                ui,
                &mut self.pending_settings,
                &mut self.session,
                &mut self.editor,
                &mut self.dirty,
                &mut self.solve_outcome,
            );
        });
    }
}
