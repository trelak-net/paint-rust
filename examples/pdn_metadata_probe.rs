//! Generate a layered interoperability fixture with nonaligned dimensions and Unicode metadata.
use paint_rust::{
    document::{BlendMode, Document, Layer},
    files,
};
use std::path::PathBuf;
fn main() -> Result<(), String> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Usage: pdn_metadata_probe OUTPUT.pdn")?,
    );
    if path.exists() {
        return Err("Choose a new output path".into());
    }
    let mut layers = Vec::new();
    for layer in 0..5_u32 {
        let pixels = (0..17 * 13_u32)
            .map(|i| {
                [
                    (i * 17 + layer * 31) as u8,
                    (i * 7 + layer * 53) as u8,
                    (i * 11 + layer * 29) as u8,
                    [0, 1, 37, 127, 128, 254, 255][i as usize % 7],
                ]
            })
            .collect();
        let mut value = Layer::new(format!("Warstwa {layer} — Żółć 日本語 🖌"), pixels);
        value.visible = layer != 2;
        value.opacity = [255, 1, 0, 127, 254][layer as usize];
        value.blend_mode = [
            BlendMode::Normal,
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::Difference,
        ][layer as usize];
        value.is_background = layer == 0;
        value.metadata.insert(
            "fixture-label".into(),
            format!("Unicode layer {layer}: Zażółć gęślą jaźń"),
        );
        layers.push(value);
    }
    let mut doc = Document::from_layers(17, 13, layers, 3)?;
    doc.metadata.insert(
        "fixture-author".into(),
        "Original synthetic interoperability fixture".into(),
    );
    doc.metadata.insert(
        "fixture-comment".into(),
        "Line one\nLine two: Żółć 日本語 🖌".into(),
    );
    files::save_project(&doc, &path)?;
    println!("Created metadata probe: {}", path.display());
    Ok(())
}
