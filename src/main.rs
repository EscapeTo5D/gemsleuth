use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([700.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Gemsleuth 宝石推理求解器（归尘给小鱼特供版）",
        options,
        Box::new(|cc| Ok(Box::new(gemsleuth::gui::GemsleuthApp::new(cc)))),
    )
}
