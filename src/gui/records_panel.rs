//! 记录录入/编辑区(§5.1,两 Tab 共享):增、删、改、启用/禁用、嫌疑标记。

use eframe::egui;

use crate::Record;
use crate::gui::{palette, Assets, SessionState};

#[derive(Default)]
pub struct RecordEditor {
    pub slots: Vec<u8>,
    pub exact: u8,
    pub partial: u8,
    pub editing: Option<usize>, // Some(i) = 修改第 i 条
}

impl RecordEditor {
    pub fn reset(&mut self) {
        self.slots.clear();
        self.exact = 0;
        self.partial = 0;
        self.editing = None;
    }
}

pub fn show(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &mut SessionState,
    editor: &mut RecordEditor,
    dirty: &mut bool,
    suspects: &[usize],
) {
    ui.heading("记录");
    if session.records.is_empty() {
        ui.label("(暂无记录,请在下方新增)");
    }
    let mut delete = None;
    for i in 0..session.records.len() {
        let guess = session.records[i].guess.clone();
        let (exact, partial) = (session.records[i].exact, session.records[i].partial);
        ui.horizontal(|ui| {
            ui.label(format!("{}.", i + 1));
            // 禁用行整体变灰(F5 排查用);egui 0.36 移除 set_enabled,改用 add_enabled_ui
            ui.add_enabled_ui(session.records[i].enabled, |ui| {
                for &g in &guess {
                    palette::small_gem(ui, assets, g);
                }
                palette::icon_count(ui, assets, true, exact);
                palette::icon_count(ui, assets, false, partial);
                if suspects.contains(&i) {
                    ui.colored_label(egui::Color32::RED, "嫌疑");
                }
            });
        });
        // 启用/编辑/删除单独一行外右侧(避免与变灰冲突)
        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut session.records[i].enabled, "启用")
                .changed()
            {
                *dirty = true;
            }
            if ui.small_button("编辑").clicked() {
                editor.slots = guess;
                editor.exact = exact;
                editor.partial = partial;
                editor.editing = Some(i);
            }
            if ui.small_button("删除").clicked() {
                delete = Some(i);
            }
        });
    }
    if let Some(i) = delete {
        session.records.remove(i);
        match editor.editing {
            Some(e) if e == i => editor.reset(),
            Some(e) if e > i => editor.editing = Some(e - 1),
            _ => {}
        }
        *dirty = true;
    }

    ui.separator();
    ui.label(if editor.editing.is_some() { "修改记录" } else { "新增记录" });

    // 槽位显示:点击已填槽位 = 清空该槽及之后
    ui.horizontal(|ui| {
        for slot in 0..session.settings.slots {
            if let Some(&g) = editor.slots.get(slot) {
                if palette::gem_button(ui, assets, g).clicked() {
                    editor.slots.truncate(slot);
                }
            } else {
                ui.image(egui::load::SizedTexture::new(assets.unknown.id(), [32.0, 32.0]));
            }
        }
    });
    // 点击色盘依次填入空槽
    ui.horizontal(|ui| {
        ui.label("点击填入:");
        for color in 0..session.settings.colors as u8 {
            if palette::gem_button(ui, assets, color).clicked()
                && editor.slots.len() < session.settings.slots
            {
                editor.slots.push(color);
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("蓝标(位置和种类都对)");
        ui.add(
            egui::DragValue::new(&mut editor.exact)
                .range(0..=session.settings.slots as u8),
        );
        ui.label("金标(种类对位置错)");
        ui.add(
            egui::DragValue::new(&mut editor.partial)
                .range(0..=session.settings.slots as u8),
        );
    });

    let candidate = Record {
        guess: editor.slots.clone(),
        exact: editor.exact,
        partial: editor.partial,
        enabled: true,
    };
    let valid_and_full = editor.slots.len() == session.settings.slots
        && candidate.validate(&session.settings).is_ok();
    if valid_and_full {
        // 规格 F4:「改」仅覆盖宝石与两个计数,保存时保留该行启用状态
        let enabled = match editor.editing {
            Some(i) => session.records[i].enabled,
            None => true,
        };
        ui.horizontal(|ui| {
            if let Some(i) = editor.editing {
                if ui.button("保存修改").clicked() {
                    session.records[i] = Record {
                        guess: candidate.guess.clone(),
                        exact: candidate.exact,
                        partial: candidate.partial,
                        enabled,
                    };
                    editor.reset();
                    *dirty = true;
                }
                if ui.button("取消").clicked() {
                    editor.reset();
                }
            } else if ui.button("添加记录").clicked() {
                session.records.push(candidate);
                editor.reset();
                *dirty = true;
            }
        });
    } else if editor.slots.len() == session.settings.slots {
        // 已填满但计数不合法 → 红字提示,不给添加(§4.5)
        ui.colored_label(
            egui::Color32::RED,
            format!("计数不合法:{}", candidate.validate(&session.settings).unwrap_err()),
        );
    } else {
        ui.label("(请填满所有槽位)");
    }
}
