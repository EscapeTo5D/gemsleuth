//! egui 薄壳:会话状态、分阶段后台分析、Tab 切换。

use eframe::egui;

use crate::{Record, Settings};

use records_panel::RecordEditor;

pub mod assistant_panel;
pub mod analysis;
pub mod result_panel;
pub mod palette;
pub mod records_panel;
pub mod solve_panel;
pub mod assets;

pub use assets::Assets;
pub use analysis::CachedAnalysis;
use analysis::AnalysisController;

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

pub struct GemsleuthApp {
    pub session: SessionState,
    pub tab: Tab,
    pub dirty: bool,                        // 任一会话变更 → 重算
    pub analysis: AnalysisController,
    pub solve_requested: bool,
    pub font_warning: bool,
    pub assets: Assets,
    pub editor: RecordEditor,
}

impl GemsleuthApp {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        let font_warning = !install_cjk_fonts(&cc.egui_ctx);
        customize_visuals(&cc.egui_ctx);
        let session = SessionState::default();
        let mut analysis = AnalysisController::default();
        analysis.request(session.settings, session.records.clone(), &cc.egui_ctx);
        Self {
            session,
            tab: Tab::Assistant,
            dirty: false,
            analysis,
            solve_requested: false,
            font_warning,
            assets: Assets::load(&cc.egui_ctx),
            editor: RecordEditor::default(),
        }
    }

    fn refresh_if_dirty(&mut self, ctx: &egui::Context) {
        if std::mem::take(&mut self.dirty) {
            self.solve_requested = false;
            self.analysis.request(self.session.settings, self.session.records.clone(), ctx);
        }
    }

    fn show_workspace(&mut self, ui: &mut egui::Ui) {
        let full = ui.available_rect_before_wrap();
        let center = full.center().x;
        let gap = ui.spacing().item_spacing.x;
        let left_rect = egui::Rect::from_min_max(full.min, egui::pos2(center - gap, full.bottom()));
        let right_rect = egui::Rect::from_min_max(egui::pos2(center + gap, full.top()), full.max);
        // 只预留标题和六条记录的高度;窗口变高时把新增空间留给候选。
        let history_height = ui.text_style_height(&egui::TextStyle::Heading)
            + ui.spacing().item_spacing.y
            + MAX_ROUNDS as f32 * records_panel::HISTORY_ROW_HEIGHT
            + (MAX_ROUNDS - 1) as f32 * ui.spacing().item_spacing.y;
        let history_rect = egui::Rect::from_min_max(right_rect.min,
            egui::pos2(right_rect.right(), full.top() + history_height.min(full.height().max(0.0))));
        ui.painter().line_segment(
            [egui::pos2(center, full.top()), egui::pos2(center, full.bottom())],
            egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
        );

        // 先处理右侧历史记录操作,编辑选择能在本帧反映到左侧录入区。
        let mut history = ui.new_child(egui::UiBuilder::new()
            .id_salt("history_panel").max_rect(history_rect)
            .layout(egui::Layout::top_down(egui::Align::LEFT)));
        records_panel::show_history(&mut history, &self.assets, &mut self.session, &mut self.editor, &mut self.dirty);
        let candidates_top = history_rect.bottom();
        self.refresh_if_dirty(ui.ctx());

        let mut left = ui.new_child(egui::UiBuilder::new()
            .id_salt("input_and_recommendation").max_rect(left_rect)
            .layout(egui::Layout::top_down(egui::Align::LEFT)));
        egui::ScrollArea::vertical().id_salt("left_workspace_scroll")
            .max_height(left_rect.height().max(0.0)).auto_shrink([false, false])
            .show(&mut left, |ui| {
                records_panel::show_editor(ui, &self.assets, &mut self.session, &mut self.editor, &self.analysis.cached, &mut self.dirty);
                self.refresh_if_dirty(ui.ctx());
                ui.separator();
                if self.analysis.phase == analysis::AnalysisPhase::Failed && ui.button("重新分析").clicked() {
                    self.analysis.request(self.session.settings, self.session.records.clone(), ui.ctx());
                }
                match self.tab {
                    Tab::Assistant => assistant_panel::show(ui, &self.assets, &mut self.session, &mut self.editor, &mut self.dirty, &self.analysis),
                    Tab::Solve => solve_panel::show(ui, &self.assets, &self.session, &mut self.solve_requested, &self.analysis),
                }
                self.refresh_if_dirty(ui.ctx());
            });

        let candidates_rect = egui::Rect::from_min_max(
            egui::pos2(right_rect.left(), candidates_top.min(full.bottom())), right_rect.max);
        let mut candidates_ui = ui.new_child(egui::UiBuilder::new()
            .id_salt("candidates_panel").max_rect(candidates_rect)
            .layout(egui::Layout::top_down(egui::Align::LEFT)));
        candidates_ui.separator();
        if self.tab == Tab::Solve && !self.solve_requested {
            candidates_ui.strong("候选列表");
            candidates_ui.label("点击左侧「求解」查看候选");
        } else if !self.analysis.cached.ready {
            candidates_ui.strong("候选列表");
            candidates_ui.label("正在核对最新记录…");
        } else {
            let count = self.analysis.cached.candidates.len();
            candidates_block(&mut candidates_ui, &self.assets, &self.analysis.cached.candidates,
                &format!("剩余候选 {count} 个（枚举顺序，不代表概率排名）"), "workspace_candidates");
        }
        ui.advance_cursor_after_rect(full);
    }
}

/// 主题强调色(琥珀金):主按钮/选中 Tab/链接。
pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(235, 172, 58);

/// 主操作按钮(琥珀底黑字,视觉上高于普通按钮)。文字用两侧 grow 原子撑到居中。
pub fn primary_button(ui: &mut egui::Ui, label: &str, min_size: egui::Vec2) -> egui::Response {
    ui.add(
        egui::Button::new((
            egui::Atom::grow(),
            egui::RichText::new(label).strong().color(egui::Color32::BLACK),
            egui::Atom::grow(),
        ))
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

/// 计算出的候选常驻块(两结果面板共用):占据调用方给的矩形(窗口右半),
/// 左缘即窗口中央分割线(绘制一条竖线),内容从分割线起排;
/// 候选单列滚动、一行一条,宝石随栏宽放大铺满一行;高度随窗口自适应——
/// 内容少时收缩,内容多时撑满剩余空间再滚动;超过 10 个只列前 10 并提示剩余。
pub fn candidates_block(
    ui: &mut egui::Ui,
    assets: &Assets,
    candidates: &[Vec<u8>],
    title: &str,
    id_salt: &str,
) {
    ui.strong(title);
    let max_h = ui.available_height().max(0.0);
    egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .max_height(max_h)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let slots = (candidates.first().map(Vec::len).unwrap_or(0) as f32).max(1.0);
            let gap = ui.spacing().item_spacing.x;
            let avail = (ui.available_width() - 14.0).max(0.0); // 留出滚动条余量
            // 宝石尺寸随栏宽自适应,上限与推荐行的大号宝石一致(64)
            let gem = (((avail - (slots - 1.0) * gap) / slots).floor()).clamp(32.0, 64.0);
            let shown = &candidates[..candidates.len().min(10)];
            for c in shown {
                ui.horizontal(|ui| {
                    for &g in c {
                        palette::gem_sized(ui, assets, g, gem);
                    }
                });
            }
            if candidates.len() > 10 {
                ui.label(egui::RichText::new(format!(
                    "(还有 {} 个未显示,继续录入记录可缩小范围)",
                    candidates.len() - 10
                )).weak());
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

impl eframe::App for GemsleuthApp {
    /// 每帧 UI 前调用,禁止阻塞——收割后台计算结果、按需派发新任务(§5.1 实时刷新)。
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.refresh_if_dirty(ctx);
        self.analysis.poll();
        if self.analysis.is_running() {
            // 结果到达主动唤醒;此周期只更新实际等待时长,不需 8ms 忙轮询。
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
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
            self.show_workspace(ui);
        });
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    fn fixture(ctx: &egui::Context, count: usize) -> GemsleuthApp {
        install_cjk_fonts(ctx);
        customize_visuals(ctx);
        let mut session = SessionState::default();
        session.records = vec![Record::new(vec![0, 1, 2, 3], 1, 0); count];
        let mut analysis = AnalysisController::default();
        analysis.phase = analysis::AnalysisPhase::Complete;
        analysis.cached.ready = true;
        analysis.cached.candidates = crate::enumerate_space(&session.settings)[..50].to_vec();
        analysis.cached.active_records = count;
        analysis.cached.total_records = count;
        GemsleuthApp { session, analysis, tab: Tab::Assistant, dirty: false, solve_requested: false,
            font_warning: false, assets: Assets::load(ctx), editor: RecordEditor::default() }
    }

    fn render(app: &mut GemsleuthApp, ctx: &egui::Context, size: egui::Vec2, events: Vec<egui::Event>) -> Vec<(String, egui::Rect)> {
        let output = ctx.run_ui(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)), events, ..Default::default()
        }, |ui| app.show_workspace(ui));
        for shape in &output.shapes {
            if let egui::Shape::Mesh(mesh) = &shape.shape {
                let min_x = mesh.vertices.iter().map(|v| v.pos.x).fold(f32::INFINITY, f32::min);
                let max_x = mesh.vertices.iter().map(|v| v.pos.x).fold(f32::NEG_INFINITY, f32::max);
                assert!(min_x >= size.x / 2.0 || max_x <= size.x / 2.0,
                    "Image crosses the column divider: {min_x}..{max_x}, width {}", size.x);
            }
        }
        output.shapes.iter().filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some((text.galley.job.text.clone(), text.galley.rect.translate(text.pos.to_vec2()))),
            _ => None,
        }).collect()
    }

    fn find(texts: &[(String, egui::Rect)], prefix: &str) -> egui::Rect {
        texts.iter().find(|(text, _)| text.starts_with(prefix)).unwrap_or_else(|| panic!("Missing {prefix}")).1
    }

    #[test]
    fn history_is_top_right_candidates_below_and_input_left_at_both_window_sizes() {
        for size in [egui::vec2(1000.0, 700.0), egui::vec2(700.0, 500.0)] {
            let ctx = egui::Context::default();
            let mut app = fixture(&ctx, 5);
            let texts = render(&mut app, &ctx, size, vec![]);
            let history = find(&texts, "历史记录");
            let input = find(&texts, "新增记录");
            let candidates = find(&texts, "剩余候选");
            assert!(history.left() >= size.x / 2.0);
            assert!(input.right() < size.x / 2.0);
            assert!((history.top() - input.top()).abs() < 12.0);
            assert!(candidates.top() > history.bottom());
            assert!(candidates.bottom() < size.y);
            let delete = find(&texts, "删除");
            assert!(delete.right() < size.x);
        }
    }

    #[test]
    fn more_history_does_not_push_the_left_recommendation_down() {
        let mut positions = Vec::new();
        for count in [1, 5] {
            let ctx = egui::Context::default();
            let mut app = fixture(&ctx, count);
            let texts = render(&mut app, &ctx, egui::vec2(1000.0, 700.0), vec![]);
            positions.push(find(&texts, "分析完成").top());
        }
        assert!((positions[0] - positions[1]).abs() < 1.0, "{positions:?}");
    }

    #[test]
    fn empty_history_reserves_the_same_space_as_a_full_history() {
        for size in [egui::vec2(1000.0, 700.0), egui::vec2(700.0, 500.0)] {
            let mut tops = Vec::new();
            for count in [0, 1, MAX_ROUNDS] {
                let ctx = egui::Context::default();
                let mut app = fixture(&ctx, count);
                let texts = render(&mut app, &ctx, size, vec![]);
                tops.push(find(&texts, "剩余候选").top());
            }
            assert!(tops.iter().all(|top| (top - tops[0]).abs() < 1.0), "{tops:?}");
            assert!(tops[0] >= MAX_ROUNDS as f32 * records_panel::HISTORY_ROW_HEIGHT);
        }
    }

    #[test]
    fn taller_windows_give_extra_space_to_candidates_not_history() {
        let mut tops = Vec::new();
        for height in [700.0, 1000.0] {
            let ctx = egui::Context::default();
            let mut app = fixture(&ctx, MAX_ROUNDS);
            let texts = render(&mut app, &ctx, egui::vec2(1000.0, height), vec![]);
            let candidates = find(&texts, "剩余候选");
            let last_record = find(&texts, "6.");
            assert!(last_record.bottom() < candidates.top());
            assert!(candidates.top() - last_record.center().y < 64.0);
            tops.push(candidates.top());
        }
        assert!((tops[0] - tops[1]).abs() < 1.0, "{tops:?}");
    }

    #[test]
    fn history_number_is_vertically_centered_with_the_large_gems() {
        let ctx = egui::Context::default();
        let mut app = fixture(&ctx, 1);
        let output = ctx.run_ui(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0))),
            ..Default::default()
        }, |ui| app.show_workspace(ui));
        let number = output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == "1." => Some(text.pos.y + text.galley.rect.center().y),
            _ => None,
        }).unwrap();
        let gem = output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect) if rect.fill_texture_id() == app.assets.gem(0).id()
                && rect.rect.left() > 500.0 => Some(rect.rect.center().y),
            egui::Shape::Mesh(mesh) if mesh.texture_id == app.assets.gem(0).id()
                && mesh.vertices.iter().all(|v| v.pos.x > 500.0) => {
                let top = mesh.vertices.iter().map(|v| v.pos.y).fold(f32::INFINITY, f32::min);
                let bottom = mesh.vertices.iter().map(|v| v.pos.y).fold(f32::NEG_INFINITY, f32::max);
                Some((top + bottom) / 2.0)
            }
            _ => None,
        }).unwrap();
        assert!((number - gem).abs() <= 2.0, "number {number}, gem {gem}");
    }

    #[test]
    fn add_record_stays_visible_and_only_accepts_complete_input() {
        for (count, complete, expected) in [(0, false, 0), (0, true, 1), (MAX_ROUNDS, true, MAX_ROUNDS)] {
            let ctx = egui::Context::default();
            let mut app = fixture(&ctx, count);
            if complete {
                app.editor.slots = vec![0, 1, 2, 3];
            }
            let size = egui::vec2(1000.0, 700.0);
            let texts = render(&mut app, &ctx, size, vec![]);
            let pos = find(&texts, "添加记录").center();
            for pressed in [true, false] {
                render(&mut app, &ctx, size, vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                    pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::default(),
                }]);
            }
            assert_eq!(app.session.records.len(), expected);
        }
    }

    #[test]
    fn history_edit_button_opens_the_left_editor() {
        let ctx = egui::Context::default();
        let mut app = fixture(&ctx, 2);
        let size = egui::vec2(1000.0, 700.0);
        let texts = render(&mut app, &ctx, size, vec![]);
        let pos = find(&texts, "编辑").center();
        for pressed in [true, false] {
            render(&mut app, &ctx, size, vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::default(),
            }]);
        }
        assert_eq!(app.editor.editing, Some(0));
        assert_eq!(app.editor.slots, app.session.records[0].guess);
        let texts = render(&mut app, &ctx, size, vec![]);
        assert!(find(&texts, "保存修改").right() < size.x / 2.0);
    }
}
