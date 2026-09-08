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

/// 宝石磁贴按钮:图 + 对称内边距,框紧贴宝石图。
/// 全局 button_padding 为 (10,5),直接套 Button::image 会得到扁框,故局部覆盖。
fn image_tile_pad(ui: &mut egui::Ui, sized: egui::load::SizedTexture, pad: f32) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(pad, pad);
        ui.add(egui::Button::image(sized))
    })
    .inner
}

/// 编辑区大号宝石磁贴:48×48 图 + 3px 内边距 → 54×54,录入主交互更大更好点。
pub fn gem_button_big(ui: &mut egui::Ui, assets: &Assets, idx: u8) -> egui::Response {
    let sized = egui::load::SizedTexture::new(assets.gem(idx).id(), [48.0, 48.0]);
    image_tile_pad(ui, sized, 3.0).on_hover_text(name(idx))
}

/// 编辑区大号空槽位:空白深色磁贴(深色底/圆角/描边/悬停),不显示图标。
/// min_size 为含边距的外框尺寸,须为 54×54 才能与大号宝石磁贴(48 图+6 边距)完全同形。
pub fn empty_slot_button_big(ui: &mut egui::Ui) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(3.0, 3.0);
        ui.add(
            egui::Button::new(egui::RichText::new(" ").size(10.0))
                .min_size(egui::vec2(54.0, 54.0)),
        )
    })
    .inner
    .on_hover_text("空槽位:点击下方色盘填入")
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

/// 只读反馈标网格:2×2(蓝金上排,问号下排),对应真实游戏记录行的右侧图标块。
pub fn marks_grid(ui: &mut egui::Ui, assets: &Assets, exact: u8, partial: u8, slots: usize) {
    // vertical 强制两行堆叠:横向布局里嵌套 horizontal 会水平并排成 1 行
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        for chunk in marks_from_counts(exact, partial, slots).chunks(2) {
            ui.horizontal(|ui| {
                for &m in chunk {
                    ui.image(egui::load::SizedTexture::new(mark_tex(assets, m).id(), [20.0, 20.0]))
                        .on_hover_text(mark_tip(m));
                }
            });
        }
    });
}

/// 归一化反馈标排列:蓝标在前、金标随后、问号补足,与只读网格同构(蓝金同存时蓝标必在前)。
pub fn normalize_marks(marks: &mut [u8]) {
    let (exact, partial) = counts_from_marks(marks);
    let canonical = marks_from_counts(exact, partial, marks.len());
    marks.copy_from_slice(&canonical);
}

/// 点击优先性:第 i 个标仅当前一标已点亮(非问号)时可点;第一个标恒可点。
pub fn mark_clickable(marks: &[u8], i: usize) -> bool {
    i == 0 || marks[i - 1] != MARK_UNKNOWN
}

/// 可点反馈标:点击循环 问号→蓝标→金标→问号;须按序点亮——前一标还是问号时,
/// 此标置灰不可点;排列恒为 蓝标在前、金标随后、问号补足。
/// 尺寸对齐:2×(20 图+4 边距) + 行距 6 = 54,与 54×54 宝石磁贴上下底齐平。
pub fn mark_cycle_buttons(ui: &mut egui::Ui, assets: &Assets, marks: &mut [u8]) {
    normalize_marks(marks);
    let clickable: Vec<bool> = (0..marks.len()).map(|i| mark_clickable(marks, i)).collect();
    // vertical 强制两行堆叠:横向布局里嵌套 horizontal 会水平并排成 1 行
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 6.0);
        for (row, chunk) in marks.chunks_mut(2).enumerate() {
            ui.horizontal(|ui| {
                for (col, m) in chunk.iter_mut().enumerate() {
                    let allowed = clickable[row * 2 + col];
                    let sized =
                        egui::load::SizedTexture::new(mark_tex(assets, *m).id(), [20.0, 20.0]);
                    let resp = ui.scope(|ui| {
                        ui.spacing_mut().button_padding = egui::vec2(2.0, 2.0);
                        ui.add_enabled(
                            allowed,
                            egui::Button::image(sized)
                                .corner_radius(egui::CornerRadius::same(12)),
                        )
                    })
                    .inner;
                    let tip = if allowed {
                        format!("{}(点击切换)", mark_tip(*m))
                    } else {
                        "须按顺序点亮:先点击前面的反馈标".to_string()
                    };
                    if resp.on_hover_text(tip).clicked() {
                        *m = (*m + 1) % 3;
                    }
                }
            });
        }
    });
    // 切换可能把前位蓝标改成金标,重排保持蓝标恒在前
    normalize_marks(marks);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click(marks: &mut [u8], i: usize) {
        assert!(mark_clickable(marks, i), "第 {i} 个标须先点亮前一个才能点");
        marks[i] = (marks[i] + 1) % 3;
        normalize_marks(marks);
    }

    #[test]
    fn 归一化_蓝标恒在金标前() {
        let mut m = vec![MARK_PARTIAL, MARK_UNKNOWN, MARK_EXACT, MARK_PARTIAL];
        normalize_marks(&mut m);
        assert_eq!(m, vec![MARK_EXACT, MARK_PARTIAL, MARK_PARTIAL, MARK_UNKNOWN]);
    }

    #[test]
    fn 点击_按序点亮且排列恒规范() {
        let mut m = vec![MARK_UNKNOWN; 4];
        assert!(!mark_clickable(&m, 1) && !mark_clickable(&m, 3));
        click(&mut m, 0); // 问号→蓝标
        assert_eq!(m, vec![MARK_EXACT, MARK_UNKNOWN, MARK_UNKNOWN, MARK_UNKNOWN]);
        click(&mut m, 0); // 蓝标→金标
        click(&mut m, 1); // 金标后点亮的新标会被重排到金标前
        assert_eq!(m, vec![MARK_EXACT, MARK_PARTIAL, MARK_UNKNOWN, MARK_UNKNOWN]);
        click(&mut m, 0); // 蓝标→金标 → (0,2):纯金标可达
        assert_eq!(m, vec![MARK_PARTIAL, MARK_PARTIAL, MARK_UNKNOWN, MARK_UNKNOWN]);
        click(&mut m, 2); // → (1,2)
        click(&mut m, 0); // → (0,3)
        assert_eq!(m, vec![MARK_PARTIAL, MARK_PARTIAL, MARK_PARTIAL, MARK_UNKNOWN]);
        click(&mut m, 2); // 金标→问号,逐个回落
        click(&mut m, 1);
        click(&mut m, 0);
        assert_eq!(m, vec![MARK_UNKNOWN; 4]);
    }

    #[test]
    fn 点击_清空前标后后续标锁定() {
        let mut m = vec![MARK_EXACT, MARK_PARTIAL, MARK_UNKNOWN, MARK_UNKNOWN];
        assert!(mark_clickable(&m, 0) && mark_clickable(&m, 1));
        click(&mut m, 1); // 金标→问号
        assert_eq!(m, vec![MARK_EXACT, MARK_UNKNOWN, MARK_UNKNOWN, MARK_UNKNOWN]);
        assert!(mark_clickable(&m, 1) && !mark_clickable(&m, 2));
    }
}
