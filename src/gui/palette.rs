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

pub fn icon_count(ui: &mut egui::Ui, assets: &Assets, exact: bool, n: u8) {
    let tex = if exact { &assets.exact } else { &assets.partial };
    let tip = if exact { "蓝标:位置和颜色都对" } else { "金标:颜色对、位置错" };
    ui.image(egui::load::SizedTexture::new(tex.id(), [20.0, 20.0]))
        .on_hover_text(tip);
    ui.label(format!("×{n}"));
}
