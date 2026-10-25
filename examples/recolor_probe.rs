//! Generate an original color/alpha ramp for external recolor comparison.
use paint_rust::{document::Document, files};
fn main() -> Result<(), String> {
    let path = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Usage: recolor_probe NEW_OUTPUT.pdn")?,
    );
    if path.exists() {
        return Err("Choose a new output path".into());
    }
    let pixels = (0..350 * 50)
        .map(|i| {
            let column = (i % 350 / 50) as u8;
            let alpha = [0, 64, 128, 200, 255][i / 350 / 10];
            [80 + column * 20, 40 + column * 20, 20 + column * 20, alpha]
        })
        .collect();
    files::save_project(&Document::from_pixels(350, 50, pixels)?, &path)
}
