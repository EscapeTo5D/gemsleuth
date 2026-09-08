//! 记录录入/编辑区(§5.1,两 Tab 共享):增、删、改、启用/禁用、嫌疑标记。
//! 规则:最多 MAX_ROUNDS 轮猜测,轮次用完后此区切换为最终答案提交。

use eframe::egui;

use crate::Record;
use crate::gui::{palette, primary_button, Assets, CachedAnalysis, MAX_ROUNDS, SessionState};

#[derive(Default)]
pub struct RecordEditor {
    pub slots: Vec<u8>,
    pub marks: Vec<u8>, // 逐槽反馈标(palette::MARK_*),蓝标/金标计数由它得出
    pub editing: Option<usize>, // Some(i) = 修改第 i 条
}

impl RecordEditor {
    pub fn reset(&mut self) {
        self.slots.clear();
        self.marks.clear();
        self.editing = None;
    }
}

pub fn show(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &mut SessionState,
    editor: &mut RecordEditor,
    cached: &CachedAnalysis,
    dirty: &mut bool,
) {
    ui.heading("记录");
    if session.records.is_empty() {
        ui.label(egui::RichText::new(
            format!("暂无记录;在下方录入第一条(共 {MAX_ROUNDS} 轮猜测机会),陪玩助手会实时给出候选与推荐"),
        ).weak());
    } else {
        // 单行一条(开关 + 宝石 + 计数 + 操作),列表放滚动区,长列表不挤压下方编辑器
        egui::ScrollArea::vertical()
            .id_salt("records_list")
            .max_height(200.0)
            .show(ui, |ui| {
                let mut delete = None;
                let can_edit = session.records.len() < MAX_ROUNDS;
                for (i, rec) in session.records.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        if ui
                            .add(egui::Checkbox::new(&mut rec.enabled, ""))
                            .on_hover_text("禁用后此记录不参与过滤(矛盾排查)")
                            .changed()
                        {
                            *dirty = true;
                        }
                        ui.label(format!("{}.", i + 1));
                        // 禁用行宝石与计数变灰;开关和操作按钮保持可点
                        ui.add_enabled_ui(rec.enabled, |ui| {
                            for &g in &rec.guess {
                                palette::small_gem(ui, assets, g);
                            }
                            // 反馈标按真实游戏排版:2 列图标块(蓝金上排,问号下排)
                            palette::marks_grid(ui, assets, rec.exact, rec.partial, rec.guess.len());
                            if cached.suspects.contains(&i) {
                                ui.colored_label(egui::Color32::RED, "嫌疑");
                            }
                        });
                        // 轮次已用满时不再进入编辑(编辑区已切换为答案提交)
                if can_edit && ui.small_button("编辑").clicked() {
                    editor.slots = rec.guess.clone();
                    editor.marks =
                        palette::marks_from_counts(rec.exact, rec.partial, rec.guess.len());
                    editor.editing = Some(i);
                }
                        if ui.small_button("删除").clicked() {
                            delete = Some(i);
                        }
                    });
                }
                if let Some(i) = delete {
                    session.records.remove(i);
                    session.answer.clear();
                    match editor.editing {
                        Some(e) if e == i => editor.reset(),
                        Some(e) if e > i => editor.editing = Some(e - 1),
                        _ => {}
                    }
                    *dirty = true;
                }
            });
    }

    ui.add_space(6.0);
    if session.records.len() >= MAX_ROUNDS {
        answer_editor(ui, assets, session, cached);
        return;
    }

    ui.strong(if editor.editing.is_some() {
        "修改记录".to_string()
    } else {
        format!("新增记录(第 {} / {MAX_ROUNDS} 轮)", session.records.len() + 1)
    });

    // 反馈标与槽位等长(默认全部问号;轮次用满切答案提交前不会走到这里)
    if editor.marks.len() != session.settings.slots {
        editor.marks.resize(session.settings.slots, palette::MARK_UNKNOWN);
    }

    // 槽位显示:点击已填槽位 = 清空该槽及之后;右侧反馈标点击循环 问号→蓝标→金标
    ui.horizontal(|ui| {
        for slot in 0..session.settings.slots {
            if let Some(&g) = editor.slots.get(slot) {
                if palette::gem_button(ui, assets, g)
                    .on_hover_text("点击清空此槽及之后")
                    .clicked()
                {
                    editor.slots.truncate(slot);
                }
            } else {
                palette::empty_slot_button(ui, assets);
            }
        }
        ui.separator();
        palette::mark_cycle_buttons(ui, assets, &mut editor.marks);
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

    let (exact, partial) = palette::counts_from_marks(&editor.marks);
    let candidate = Record { guess: editor.slots.clone(), exact, partial, enabled: true };
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
                if primary_button(ui, "保存修改", egui::vec2(110.0, 30.0)).clicked() {
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
            } else if primary_button(ui, "添加记录", egui::vec2(110.0, 30.0)).clicked() {
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
        ui.label(egui::RichText::new("(请填满所有槽位)").weak());
    }
}

/// 第 MAX_ROUNDS+1 轮:提交最终答案。填满即自动判卷,无需按钮。
/// 判卷依据:答案 ∈ 剩余候选(与全部启用记录一致);候选唯一则推理必正确。
fn answer_editor(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &mut SessionState,
    cached: &CachedAnalysis,
) {
    ui.strong(format!("第 {} 轮:提交最终答案", MAX_ROUNDS + 1));
    ui.horizontal(|ui| {
        for slot in 0..session.settings.slots {
            if let Some(&g) = session.answer.get(slot) {
                if palette::gem_button(ui, assets, g)
                    .on_hover_text("点击清空此槽及之后")
                    .clicked()
                {
                    session.answer.truncate(slot);
                }
            } else {
                palette::empty_slot_button(ui, assets);
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("点击填入:");
        for color in 0..session.settings.colors as u8 {
            if palette::gem_button(ui, assets, color).clicked()
                && session.answer.len() < session.settings.slots
            {
                session.answer.push(color);
            }
        }
        if !session.answer.is_empty() && ui.small_button("清空").clicked() {
            session.answer.clear();
        }
    });
    if session.answer.len() < session.settings.slots {
        ui.label(egui::RichText::new("(请填满所有槽位)").weak());
        return;
    }
    let full = session.answer.clone();
    // 候选 = 与全部启用记录一致的组合,故"在候选中"等价于"与记录一致"
    if cached.candidates.iter().any(|c| c == &full) {
        if cached.candidates.len() == 1 {
            ui.colored_label(
                egui::Color32::from_rgb(102, 187, 106),
                "答案成立,且是唯一可能——推理正确!",
            );
        } else {
            ui.colored_label(
                crate::gui::ACCENT,
                format!(
                    "答案与全部记录一致,但仍有 {} 个候选同样成立,未必正确",
                    cached.candidates.len()
                ),
            );
        }
    } else {
        ui.colored_label(egui::Color32::RED, "答案与记录矛盾(不在剩余候选中)");
    }
}
