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
    pub dirty: bool,                        // 任一会话变更 → 重算
    pub cached: CachedAnalysis,
    /// 后台重算状态:脏标记只发请求,计算在独立线程完成经 channel 送回,
    /// 期间 UI 持续用上一份 cached 渲染(显示"计算中"),不再整帧卡死。
    pub compute_gen: u64,                                        // 最新任务代号
    pub compute_rx: Option<std::sync::mpsc::Receiver<(u64, CachedAnalysis)>>,
    pub computing: bool,                   // 最新代号的任务尚未返回
    pub compute_pending: bool,             // 任务执行期间又有变更,收割后需补算
    pub solve_outcome: Option<SolveOutcome>, // 整卷求解快照(点击求解时更新)
    pub font_warning: bool,
    pub assets: Assets,
    pub editor: RecordEditor,
}

/// 由会话快照计算完整分析(过滤 + 推荐/嫌疑)。UI 线程启动时与后台线程共用。
fn compute_cached(session: &SessionState) -> CachedAnalysis {
    compute_cached_from(session.settings, session.records.clone())
}

fn compute_cached_from(settings: crate::Settings, records: Vec<crate::Record>) -> CachedAnalysis {
    let candidates = crate::filter_candidates(&settings, &records);
    let (recommendation, suspects) = if candidates.is_empty() {
        (None, crate::suspect_records(&settings, &records))
    } else {
        (Some(crate::core::strategy::recommend_for(&settings, &candidates)), vec![])
    };
    CachedAnalysis { candidates, recommendation, suspects }
}

impl GemsleuthApp {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        let font_warning = !install_cjk_fonts(&cc.egui_ctx);
        customize_visuals(&cc.egui_ctx);
        let session = SessionState::default();
        // 启动时记录为空,计算毫秒级,直接同步出首帧数据
        let cached = compute_cached(&session);
        Self {
            session,
            tab: Tab::Assistant,
            dirty: false,
            cached,
            compute_gen: 0,
            compute_rx: None,
            computing: false,
            compute_pending: false,
            solve_outcome: None,
            font_warning,
            assets: Assets::load(&cc.egui_ctx),
            editor: RecordEditor::default(),
        }
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

/// 推荐猜测展示行(两结果面板共用,§5.1)。标签独立成行,宝石一行,备注文字另起一行(窄栏不挤压)。
pub fn recommendation_row(ui: &mut egui::Ui, assets: &Assets, rec: &Recommendation) {
    ui.strong("推荐下一猜:");
    match rec {
        Recommendation::Answer(ans) => {
            ui.horizontal(|ui| {
                for &g in ans {
                    palette::big_gem(ui, assets, g);
                }
            });
        }
        Recommendation::Guess { guess, bound } => {
            ui.horizontal(|ui| {
                for &g in guess {
                    palette::big_gem(ui, assets, g);
                }
            });
            let note = match bound {
                Bound::GuaranteedSteps(n) => format!("(精确前瞻:最多还需 {n} 步)"),
                Bound::Expected { entropy_bits, worst_bucket } => format!(
                    "(熵推荐:期望信息量 {entropy_bits:.2} 比特,最坏情况剩 {worst_bucket} 个)"
                ),
            };
            ui.label(egui::RichText::new(note).weak());
        }
    }
}

/// 计算出的候选常驻块(两结果面板共用):占据调用方给的矩形(窗口右半),
/// 左缘即窗口中央分割线(绘制一条竖线),内容从分割线起排;
/// 候选单列滚动、一行一条,宝石随栏宽放大铺满一行;高度随窗口自适应——
/// 内容少时收缩,内容多时撑满剩余空间再滚动;超过 50 个只列前 50 并提示剩余。
pub fn candidates_block(
    ui: &mut egui::Ui,
    assets: &Assets,
    candidates: &[Vec<u8>],
    title: &str,
    id_salt: &str,
) {
    // 左缘竖直分割线(窗口中央)
    let r = ui.max_rect();
    ui.painter().line_segment(
        [egui::pos2(r.left(), r.top()), egui::pos2(r.left(), r.bottom())],
        egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
    );
    ui.strong(title);
    let max_h = ui.available_height().max(160.0);
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
            let shown = &candidates[..candidates.len().min(50)];
            for c in shown {
                ui.horizontal(|ui| {
                    for &g in c {
                        palette::gem_sized(ui, assets, g, gem);
                    }
                });
            }
            if candidates.len() > 50 {
                ui.label(egui::RichText::new(format!(
                    "(还有 {} 个未显示,继续录入记录可缩小范围)",
                    candidates.len() - 50
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
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 收割已完成的任务;只采纳最新代号,过期结果直接丢弃
        if let Some(rx) = &self.compute_rx {
            while let Ok((job_gen, result)) = rx.try_recv() {
                if job_gen == self.compute_gen {
                    self.cached = result;
                    self.computing = false;
                }
            }
        }
        if !self.computing && self.compute_pending {
            // 在跑的任务已收尾且期间有变更 → 对最新状态补算
            self.compute_pending = false;
            self.dirty = true;
        }
        if !self.dirty {
            return;
        }
        if self.computing {
            // 任务执行期间又有变更:合并请求,收割后再对最新状态补算
            self.compute_pending = true;
            return;
        }
        self.dirty = false;
        self.compute_gen += 1;
        let job_gen = self.compute_gen;
        let settings = self.session.settings;
        let records = self.session.records.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.compute_rx = Some(rx);
        self.computing = true;
        std::thread::Builder::new()
            .name("recompute".into())
            .spawn(move || {
                let analysis = compute_cached_from(settings, records);
                let _ = tx.send((job_gen, analysis));
            })
            .expect("spawn recompute thread");
        // 任务在跑期间保持低间隔轮询收割,否则空闲时无输入事件不会出新帧
        if self.computing {
            _ctx.request_repaint_after(std::time::Duration::from_millis(8));
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
                    assistant_panel::show(
                        ui,
                        &self.assets,
                        &mut self.session,
                        &mut self.editor,
                        &mut self.dirty,
                        &self.cached,
                        self.computing,
                    )
                }
                Tab::Solve => solve_panel::show(
                    ui,
                    &self.assets,
                    &self.session,
                    &mut self.solve_outcome,
                    &self.cached.suspects,
                ),
            }
        });
    }
}
