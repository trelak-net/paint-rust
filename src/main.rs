mod app;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Paint Rust")
            .with_inner_size([1180.0, 800.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Paint Rust",
        options,
        Box::new(|cc| Ok(Box::new(app::Editor::new(cc)))),
    )
}
