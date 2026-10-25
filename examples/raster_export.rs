//! Export a decoded image to a new raster path for independent codec comparisons.
use paint_rust::files::{self, RasterFormat};
fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let input =
        std::path::PathBuf::from(args.next().ok_or("Usage: raster_export INPUT NEW_OUTPUT")?);
    let output =
        std::path::PathBuf::from(args.next().ok_or("Usage: raster_export INPUT NEW_OUTPUT")?);
    if output.exists() {
        return Err("Choose a new output path".into());
    }
    let format = RasterFormat::from_path(&output).ok_or("Unsupported output extension")?;
    let document = files::open(&input)?;
    let bytes = files::encode_raster_bytes(
        document.pixels(),
        (document.width, document.height),
        files::RasterOptions {
            dpi: document.resolution(),
            ..files::RasterOptions::new(format)
        },
        &std::sync::atomic::AtomicBool::new(false),
    )?;
    files::save_encoded_raster(&bytes, &output, &std::sync::atomic::AtomicBool::new(false))
}
