//! 颜色索引 → 名称/素材映射(§5.2)。颜色语义只存在于这一层。

use eframe::egui;

use crate::gui::assets::Assets;

pub const NAMES: [&str; 6] = ["红", "蓝", "紫", "橙", "黄", "绿"];

/// 颜色数固定为 6(红/蓝/紫/橙/黄/绿)。核心引擎仍支持 4..=8,仅 GUI 固定。
pub const COLORS: usize = 6;

pub fn name(idx: u8) -> &'static str {
    NAMES[idx as usize]
}

pub fn small_gem(ui: &mut egui::Ui, assets: &Assets, idx: u8) {
    ui.image(egui::load::SizedTexture::new(assets.gem(idx).id(), [32.0, 32.0]));
}

pub fn big_gem(ui: &mut egui::Ui, assets: &Assets, idx: u8) {
    ui.image(egui::load::SizedTexture::new(assets.gem(idx).id(), [64.0, 64.0]));
}

/// 宝石磁贴按钮:32×32 图 + 2px 对称内边距 → 36×36,框紧贴宝石图。
/// 全局 button_padding 为 (10,5),直接套 Button::image 会得到 52×42 的扁框,故局部覆盖。
fn image_tile(ui: &mut egui::Ui, sized: egui::load::SizedTexture) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(2.0, 2.0);
        ui.add(egui::Button::image(sized))
    })
    .inner
}

pub fn gem_button(ui: &mut egui::Ui, assets: &Assets, idx: u8) -> egui::Response {
    let sized = egui::load::SizedTexture::new(assets.gem(idx).id(), [32.0, 32.0]);
    image_tile(ui, sized).on_hover_text(name(idx))
}

/// 空槽位:与色盘宝石按钮同款磁贴(深色底/圆角/描边/悬停),内部为空。
pub fn empty_slot_button(ui: &mut egui::Ui, assets: &Assets) -> egui::Response {
    let sized = egui::load::SizedTexture::new(assets.unknown.id(), [32.0, 32.0]);
    image_tile(ui, sized).on_hover_text("空槽位:点击下方色盘填入")
}

/// 反馈标三态(与真实游戏一致):问号=无反馈,蓝标=位置和种类都对,金标=种类对位置错。
pub const MARK_UNKNOWN: u8 = 0;
pub const MARK_EXACT: u8 = 1;
pub const MARK_PARTIAL: u8 = 2;

fn mark_tex(assets: &Assets, mark: u8) -> &egui::TextureHandle {
    match mark {
        MARK_EXACT => &assets.exact,
        MARK_PARTIAL => &assets.partial,
        _ => &assets.unknown,
    }
}

fn mark_tip(mark: u8) -> &'static str {
    match mark {
        MARK_EXACT => "蓝标:宝石种类和位置都对",
        MARK_PARTIAL => "金标:种类对但位置错",
        _ => "问号:此槽无反馈标",
    }
}

/// 由两个计数还原逐槽反馈标:蓝标在前、金标随后、问号补足(对应游戏排版:蓝金上排,问号下排)。
pub fn marks_from_counts(exact: u8, partial: u8, slots: usize) -> Vec<u8> {
    let mut marks = vec![MARK_EXACT; exact as usize];
    marks.extend(std::iter::repeat(MARK_PARTIAL).take(partial as usize));
    marks.resize(slots, MARK_UNKNOWN);
    marks
}

/// 逐槽反馈标 → (蓝标数, 金标数)。
pub fn counts_from_marks(marks: &[u8]) -> (u8, u8) {
    let n = |want| marks.iter().filter(|&&m| m == want).count() as u8;
    (n(MARK_EXACT), n(MARK_PARTIAL))
}

/// 只读反馈标网格:每行 2 枚 20×20,对应真实游戏记录行的右侧图标块。
pub fn marks_grid(ui: &mut egui::Ui, assets: &Assets, exact: u8, partial: u8, slots: usize) {
    for chunk in marks_from_counts(exact, partial, slots).chunks(2) {
        ui.horizontal(|ui| {
            for &m in chunk {
                ui.image(egui::load::SizedTexture::new(mark_tex(assets, m).id(), [20.0, 20.0]))
                    .on_hover_text(mark_tip(m));
            }
        });
    }
}

/// 可点反馈标:点击循环 问号→蓝标→金标,每行 2 枚,替代原数字计数输入。
pub fn mark_cycle_buttons(ui: &mut egui::Ui, assets: &Assets, marks: &mut [u8]) {
    for chunk in marks.chunks_mut(2) {
        ui.horizontal(|ui| {
            for m in chunk {
                let sized = egui::load::SizedTexture::new(mark_tex(assets, *m).id(), [20.0, 20.0]);
                let resp = image_tile(ui, sized);
                if resp.on_hover_text(format!("{}(点击切换)", mark_tip(*m))).clicked() {
                    *m = (*m + 1) % 3;
                }
            }
        });
    }
}
