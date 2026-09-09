//! 记录录入/编辑区(§5.1,两 Tab 共享):增、删、改、启用/禁用、嫌疑标记。
//! 规则:最多 MAX_ROUNDS 轮猜测,轮次用完后此区切换为最终答案提交。

use eframe::egui;

use crate::Record;
use crate::gui::{palette, Assets, CachedAnalysis, MAX_ROUNDS, SessionState};

pub const HISTORY_ROW_HEIGHT: f32 = 48.0;

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

pub fn show_history(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &mut SessionState,
    editor: &mut RecordEditor,
    dirty: &mut bool,
) {
    ui.heading("历史记录");
    if session.records.is_empty() {
        ui.label(egui::RichText::new(
            format!("暂无记录，请在左侧录入（共 {MAX_ROUNDS} 轮猜测机会）"),
        ).weak());
    } else {
        // 单行一条(开关 + 宝石 + 计数 + 操作),列表放滚动区,长列表不挤压下方编辑器
        egui::ScrollArea::vertical()
            .id_salt("records_list")
            .max_height(ui.available_height().max(0.0))
            .show(ui, |ui| {
                let mut delete = None;
                let can_edit = session.records.len() < MAX_ROUNDS;
                for (i, rec) in session.records.iter_mut().enumerate() {
                    // 先确定 48px 行高,让先绘制的勾选与序号也按宝石高度居中。
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), HISTORY_ROW_HEIGHT),
                        egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true),
                        |ui| {
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
                                palette::gem_sized(ui, assets, g, HISTORY_ROW_HEIGHT);
                            }
                            // 反馈标按真实游戏排版:2 列图标块(蓝金上排,问号下排)
                            palette::marks_grid(ui, assets, rec.exact, rec.partial, rec.guess.len());
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

}

pub fn show_editor(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &mut SessionState,
    editor: &mut RecordEditor,
    cached: &CachedAnalysis,
    dirty: &mut bool,
) {
    if session.records.len() >= MAX_ROUNDS {
        answer_editor(ui, assets, session, cached, !*dirty);
        record_button(ui, "添加记录", false)
            .on_disabled_hover_text("已达到六条记录上限");
        if cached.ready && !*dirty {
            super::suspects_row(ui, &cached.suspects);
        }
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

    // 槽位显示:点击已填槽位 = 清空该槽及之后;右侧反馈标按序点亮(前一标非问号才能点下一个),蓝标恒在金标前
    ui.horizontal(|ui| {
        for slot in 0..session.settings.slots {
            if let Some(&g) = editor.slots.get(slot) {
                if palette::gem_button_big(ui, assets, g)
                    .on_hover_text("点击清空此槽及之后")
                    .clicked()
                {
                    editor.slots.truncate(slot);
                }
            } else {
                palette::empty_slot_button_big(ui);
            }
        }
        ui.separator();
        palette::mark_cycle_buttons(ui, assets, &mut editor.marks);
    });
        // 常显图例:两个彩色反馈标的含义(细节仍在悬浮提示)
        ui.horizontal_wrapped(|ui| {
            ui.horizontal(|ui| {
                ui.image(egui::load::SizedTexture::new(assets.exact.id(), [16.0, 16.0]));
                ui.label(egui::RichText::new("位置和颜色都对").weak().small());
            });
            ui.horizontal(|ui| {
                ui.image(egui::load::SizedTexture::new(assets.partial.id(), [16.0, 16.0]));
                ui.label(egui::RichText::new("颜色对、位置错").weak().small());
            });
        });
    // 点击色盘依次填入空槽(标签独立一行,色盘另起一行)
    ui.label("点击填入:");
    ui.horizontal_wrapped(|ui| {
        for color in 0..session.settings.colors as u8 {
            if palette::gem_button_big(ui, assets, color).clicked()
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
    {
        // 规格 F4:「改」仅覆盖宝石与两个计数,保存时保留该行启用状态
        let enabled = match editor.editing {
            Some(i) => session.records[i].enabled,
            None => true,
        };
        ui.horizontal(|ui| {
            if let Some(i) = editor.editing {
                if record_button(ui, "保存修改", valid_and_full).clicked() {
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
            } else if record_button(ui, "添加记录", valid_and_full).clicked() {
                session.records.push(candidate.clone());
                editor.reset();
                *dirty = true;
            }
        });
    }
    if !valid_and_full && editor.slots.len() == session.settings.slots {
        // 已填满但计数不合法 → 红字提示,不给添加(§4.5)
        ui.colored_label(
            egui::Color32::RED,
            format!("计数不合法:{}", candidate.validate(&session.settings).unwrap_err()),
        );
    } else if !valid_and_full {
        ui.label(egui::RichText::new("(请填满所有槽位)").weak());
    }
    // 包括保存编辑在内的所有操作结束后再展示,避免本帧的旧嫌疑结论。
    if cached.ready && !*dirty {
        super::suspects_row(ui, &cached.suspects);
    }
}

/// 第 MAX_ROUNDS+1 轮:提交最终答案。填满即自动判卷,无需按钮。
/// 判卷依据:答案 ∈ 剩余候选(与全部启用记录一致);候选唯一则推理必正确。
fn answer_editor(
    ui: &mut egui::Ui,
    assets: &Assets,
    session: &mut SessionState,
    cached: &CachedAnalysis,
    current: bool,
) {
    ui.strong(format!("第 {} 轮:提交最终答案", MAX_ROUNDS + 1));
    ui.horizontal(|ui| {
        for slot in 0..session.settings.slots {
            if let Some(&g) = session.answer.get(slot) {
                if palette::gem_button_big(ui, assets, g)
                    .on_hover_text("点击清空此槽及之后")
                    .clicked()
                {
                    session.answer.truncate(slot);
                }
            } else {
                palette::empty_slot_button_big(ui);
            }
        }
    });
    ui.label("点击填入:");
    ui.horizontal_wrapped(|ui| {
        for color in 0..session.settings.colors as u8 {
            if palette::gem_button_big(ui, assets, color).clicked()
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
    match answer_verdict(cached, current, &full) {
        AnswerVerdict::Pending => {
            ui.label("正在核对最新记录，候选更新后自动校验答案…");
        }
        AnswerVerdict::Unique => {
            ui.colored_label(
                egui::Color32::from_rgb(102, 187, 106),
                "在已启用记录准确的前提下，这是唯一符合记录的答案",
            );
        }
        AnswerVerdict::Possible(count) => {
            ui.colored_label(
                crate::gui::ACCENT,
                format!(
                    "答案与全部启用记录一致,但仍有 {} 个候选同样成立,尚不能确定正确",
                    count
                ),
            );
        }
        AnswerVerdict::Contradiction => {
            ui.colored_label(egui::Color32::RED, "答案与启用记录矛盾(不在剩余候选中)");
        }
    }
}

fn record_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    let (fill, text) = if enabled {
        (super::ACCENT, egui::Color32::BLACK)
    } else {
        (egui::Color32::from_gray(55), egui::Color32::from_gray(145))
    };
    ui.add_enabled(enabled, egui::Button::new((
        egui::Atom::grow(),
        egui::RichText::new(label).strong().size(14.0).color(text),
        egui::Atom::grow(),
    )).fill(fill).corner_radius(6).min_size(egui::vec2(120.0, 32.0)))
}

#[derive(Debug, PartialEq)]
enum AnswerVerdict { Pending, Unique, Possible(usize), Contradiction }

fn answer_verdict(cached: &CachedAnalysis, current: bool, answer: &[u8]) -> AnswerVerdict {
    if !current || !cached.ready {
        return AnswerVerdict::Pending;
    }
    if !cached.candidates.iter().any(|candidate| candidate == answer) {
        return AnswerVerdict::Contradiction;
    }
    if cached.candidates.len() == 1 { AnswerVerdict::Unique } else { AnswerVerdict::Possible(cached.candidates.len()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_records_never_validates_against_an_old_unique_answer() {
        let cached = CachedAnalysis { ready: true, candidates: vec![vec![2, 2, 2, 4]], ..Default::default() };
        assert_eq!(answer_verdict(&cached, true, &[2, 2, 2, 4]), AnswerVerdict::Unique);
        assert_eq!(answer_verdict(&cached, false, &[2, 2, 2, 4]), AnswerVerdict::Pending);
        assert_eq!(answer_verdict(&cached, false, &[0; 4]), AnswerVerdict::Pending);
    }

    #[test]
    fn unfiltered_cache_is_not_a_contradiction() {
        let cached = CachedAnalysis::default();
        assert_eq!(answer_verdict(&cached, true, &[0; 4]), AnswerVerdict::Pending);
        let filtered = CachedAnalysis { ready: true, ..Default::default() };
        assert_eq!(answer_verdict(&filtered, true, &[0; 4]), AnswerVerdict::Contradiction);
    }

    #[test]
    fn multiple_matching_answers_are_not_presented_as_unique() {
        let cached = CachedAnalysis { ready: true, candidates: vec![vec![1; 4], vec![2; 4]], ..Default::default() };
        assert_eq!(answer_verdict(&cached, true, &[1; 4]), AnswerVerdict::Possible(2));
        assert_eq!(answer_verdict(&cached, true, &[3; 4]), AnswerVerdict::Contradiction);
    }
}
