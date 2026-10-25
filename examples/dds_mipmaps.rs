//! Write an original RGBA texture with a full mip chain for independent inspection.
use paint_rust::{dds, document::Document};
fn main() -> Result<(), String> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("Usage: dds_mipmaps NEW_OUTPUT.dds")?;
    let writer = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let pixels = (0..32 * 16)
        .map(|i| {
            [
                (i % 32 * 8) as u8,
                (i / 32 * 16) as u8,
                80,
                if i % 32 < 8 { 64 } else { 255 },
            ]
        })
        .collect();
    let document = Document::from_pixels(32, 16, pixels)?;
    dds::encode_with_mipmaps(
        document.pixels(),
        (32, 16),
        writer,
        true,
        &std::sync::atomic::AtomicBool::new(false),
    )
}
