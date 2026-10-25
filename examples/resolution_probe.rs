//! Create a synthetic layered document for physical-resolution interoperability checks.
use paint_rust::{document::Document, files, resolution::Dpi};
fn main() -> Result<(), String> {
    let path = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Usage: resolution_probe OUTPUT.pdn")?,
    );
    if path.exists() {
        return Err("Choose a new output path".into());
    }
    let mut document = Document::new(300, 150, [40, 80, 120, 255])?;
    document.set_resolution(Dpi { x: 300.0, y: 300.0 })?;
    files::save_project(&document, &path)?;
    println!("Created 300 DPI fixture: {}", path.display());
    Ok(())
}
