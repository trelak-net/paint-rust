//! Render original multilingual text using the installed outline fallback catalog.
use paint_rust::{
    files,
    text::{self, TextStyle, fallback::Resolver},
};
use std::{path::PathBuf, sync::atomic::AtomicBool};
fn main() -> Result<(), String> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Usage: text_fallback NEW_OUTPUT.png")?,
    );
    if path.exists() {
        return Err("Choose a new output path".into());
    }
    let definitions = eframe::egui::FontDefinitions::default();
    let primary =
        ab_glyph::FontArc::try_from_vec(definitions.font_data["Ubuntu-Light"].font.to_vec())
            .map_err(|e| e.to_string())?;
    let content = "Paint Rust — Zażółć gęślą jaźń\nArabic: مرحبا بالعالم\nHebrew: שלום עולם\nCombining marks: cafe\u{301} — Emoji: 😀";
    let cancel = AtomicBool::new(false);
    let mut resolver = Resolver::default();
    if !resolver.prepare(&primary, 0, content, &cancel) {
        return Err("Font fallback was interrupted".into());
    }
    let mut pixels = vec![[255; 4]; 900 * 280];
    if !text::render_with_fallback_cancellable(
        &mut pixels,
        900,
        280,
        None,
        TextStyle {
            layout: Default::default(),
            font: &primary,
            text: content,
            origin: (20.0, 20.0),
            size: 48.0,
            color: [24, 32, 48, 255],
        },
        resolver.fonts(),
        &cancel,
    ) {
        return Err("Text rendering was interrupted".into());
    }
    let bytes = files::encode_raster_bytes(
        &pixels,
        (900, 280),
        files::RasterOptions::new(files::RasterFormat::Png),
        &cancel,
    )?;
    files::save_encoded_raster(&bytes, &path, &cancel)?;
    println!(
        "Loaded {} fallback faces; wrote {}",
        resolver.fonts().len(),
        path.display()
    );
    Ok(())
}
