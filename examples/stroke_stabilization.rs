//! Render an original pointer-jitter probe with stabilization off and enabled.
use paint_rust::{brush::Options, document::Document, files};
use std::{path::PathBuf, sync::atomic::AtomicBool};
fn main() -> Result<(), String> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Usage: stroke_stabilization NEW_OUTPUT.png")?,
    );
    if path.exists() {
        return Err("Choose a new output path".into());
    }
    let mut document = Document::new(640, 220, [255; 4])?;
    for (row, smoothing) in [(60.0, 0.0), (160.0, 16.0)] {
        let points: Vec<_> = (0..70)
            .map(|i| {
                let x = 30.0 + i as f32 * 8.0;
                (
                    x,
                    row + (i as f32 * 0.12).sin() * 16.0 + if i % 2 == 0 { -4.0 } else { 4.0 },
                )
            })
            .collect();
        document.begin("Probe stroke");
        for pair in points.windows(2) {
            document.brush_segment(
                pair[0],
                pair[1],
                4.0,
                [30, 90, 180, 255],
                Options {
                    smoothing,
                    antialias: true,
                    ..Default::default()
                },
                false,
            );
        }
        document.commit();
    }
    let cancel = AtomicBool::new(false);
    let bytes = files::encode_raster_bytes(
        document.pixels(),
        (640, 220),
        files::RasterOptions::new(files::RasterFormat::Png),
        &cancel,
    )?;
    files::save_encoded_raster(&bytes, &path, &cancel)
}
