//! Generate a small layered document for external merge-down interoperability checks.
use paint_rust::{
    document::{BlendMode, Document, Layer},
    files,
};
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Usage: pdn_merge_probe OUTPUT.pdn [--all-modes]")?,
    );
    if path.exists() {
        return Err("Choose a new output path".into());
    }
    let mut bottom = Layer::new(
        "Lower properties retained".into(),
        vec![
            [0, 0, 255, 255],
            [80, 120, 200, 255],
            [20, 70, 120, 140],
            [240, 160, 80, 0],
            [120, 120, 120, 64],
            [255; 4],
        ],
    );
    bottom.visible = false;
    bottom.opacity = 123;
    bottom.blend_mode = BlendMode::Screen;
    bottom
        .metadata
        .insert("probe".into(), "lower metadata".into());
    let mut top = Layer::new(
        "Merge this layer down".into(),
        vec![
            [255, 0, 0, 255],
            [160, 80, 40, 255],
            [210, 160, 40, 180],
            [70, 150, 230, 128],
            [0; 4],
            [30, 60, 90, 200],
        ],
    );
    top.visible = false;
    top.opacity = 173;
    top.blend_mode = BlendMode::Multiply;
    let all = std::env::args().nth(2).as_deref() == Some("--all-modes");
    let modes = if all {
        BlendMode::ALL.as_slice()
    } else {
        &[BlendMode::Multiply]
    };
    let mut layers = Vec::new();
    for mode in modes {
        let mut lower = bottom.clone();
        let mut upper = top.clone();
        if all {
            lower.name = format!("{mode:?} result");
        }
        upper.blend_mode = *mode;
        layers.extend([lower, upper]);
    }
    let active = layers.len() - 1;
    let doc = Document::from_layers(3, 2, layers, active)?;
    files::save_project(&doc, &path)?;
    println!("Created merge-down probe: {}", path.display());
    Ok(())
}
