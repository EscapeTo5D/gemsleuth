//! 陪玩助手结果区(§5.1):逐条增长记录,实时(无需按钮)显示候选与推荐。

use eframe::egui;

use crate::gui::{palette, recommendation_row, suspects_row, Assets, CachedAnalysis};

pub fn show(ui: &mut egui::Ui, assets: &Assets, cached: &CachedAnalysis) {
    ui.heading("陪玩助手");
    match cached.candidates.len() {
        0 => {
            ui.colored_label(egui::Color32::RED, "记录矛盾!剩余候选为 0,请检查录入。");
            suspects_row(ui, &cached.suspects);
        }
        1 => {
            ui.strong("答案确定:");
            ui.horizontal(|ui| {
                for &g in &cached.candidates[0] {
                    palette::big_gem(ui, assets, g);
                }
            });
        }
        n => {
            ui.strong(format!("剩余候选 {n} 个"));
            if let Some(rec) = &cached.recommendation {
                recommendation_row(ui, assets, rec);
            }
            if n > 50 {
                ui.label(egui::RichText::new("(候选超过 50 个,折叠显示)").weak());
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("assistant_candidates")
                    .max_height(260.0)
                    .show(ui, |ui| {
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
