//! 颜色索引 → 名称/素材映射(§5.2)。颜色语义只存在于这一层。

use eframe::egui;

use crate::gui::assets::Assets;

pub const NAMES: [&str; 8] = ["红", "蓝", "紫", "橙", "黄", "绿", "青", "白"];

/// 颜色数固定为 4(红/蓝/紫/橙)。核心引擎仍支持 4..=8,仅 GUI 固定。
pub const COLORS: usize = 4;

pub fn name(idx: u8) -> &'static str {
    NAMES[idx as usize]
}

pub fn small_gem(ui: &mut egui::Ui, assets: &Assets, idx: u8) {
    ui.image(egui::load::SizedTexture::new(assets.gem(idx).id(), [32.0, 32.0]));
}

pub fn big_gem(ui: &mut egui::Ui, assets: &Assets, idx: u8) {
    ui.image(egui::load::SizedTexture::new(assets.gem(idx).id(), [64.0, 64.0]));
}

pub fn gem_button(ui: &mut egui::Ui, assets: &Assets, idx: u8) -> egui::Response {
    let sized = egui::load::SizedTexture::new(assets.gem(idx).id(), [32.0, 32.0]);
    ui.add(egui::Button::image(sized)).on_hover_text(name(idx))
}

pub fn icon_count(ui: &mut egui::Ui, assets: &Assets, exact: bool, n: u8) {
    let tex = if exact { &assets.exact } else { &assets.partial };
    ui.image(egui::load::SizedTexture::new(tex.id(), [20.0, 20.0]));
    ui.label(format!("×{n}"));
}
