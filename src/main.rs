mod app;
mod appearance;
mod canvas_renderer;
mod clipboard;
mod icons;
mod preview;

fn main() -> eframe::Result {
    let icon = image::load_from_memory(include_bytes!("../assets/app-icon.png"))
        .expect("embedded application icon")
        .into_rgba8();
    let icon = eframe::egui::IconData {
        width: icon.width(),
        height: icon.height(),
        rgba: icon.into_raw(),
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Paint Rust")
            .with_icon(icon)
            .with_app_id("net.trelak.PaintRust")
            .with_decorations(true)
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
