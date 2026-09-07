//! 陪玩助手结果区(§5.1):逐条增长记录,实时(无需按钮)显示候选与推荐。

use eframe::egui;

use crate::gui::{palette, recommendation_row, Assets, CachedAnalysis};

pub fn show(ui: &mut egui::Ui, assets: &Assets, cached: &CachedAnalysis) {
    ui.heading("陪玩助手");
    match cached.candidates.len() {
        0 => {
            ui.colored_label(egui::Color32::RED, "记录矛盾!剩余候选为 0,请检查录入。");
            if !cached.suspects.is_empty() {
                let list =
                    cached.suspects.iter().map(|i| (i + 1).to_string()).collect::<Vec<_>>().join("、");
                ui.label(format!("嫌疑记录:第 {list} 条(禁用后候选恢复非空)"));
            }
        }
        1 => {
            ui.label("答案确定:");
            ui.horizontal(|ui| {
                for &g in &cached.candidates[0] {
                    palette::big_gem(ui, assets, g);
                }
            });
        }
        n => {
            ui.label(format!("剩余候选 {n} 个"));
            if let Some(rec) = &cached.recommendation {
                recommendation_row(ui, assets, rec);
            }
            if n > 50 {
                ui.label("(候选超过 50 个,折叠显示)");
            } else {
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    for c in &cached.candidates {
                        ui.horizontal(|ui| {
                            for &g in c {
                                palette::small_gem(ui, assets, g);
                            }
                        });
                    }
                });
            }
        }
    }
}
