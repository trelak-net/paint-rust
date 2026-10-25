//! Convert a document to PDN and verify the decoded image and metadata.
use paint_rust::files;
use std::path::PathBuf;
fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let input = PathBuf::from(args.next().ok_or("Usage: pdn_roundtrip INPUT OUTPUT.pdn")?);
    let output = PathBuf::from(args.next().ok_or("Usage: pdn_roundtrip INPUT OUTPUT.pdn")?);
    if input == output || output.exists() {
        return Err("Choose a new output path".into());
    }
    if !output
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("pdn"))
    {
        return Err("Output must have a .pdn extension".into());
    }
    let document = files::open(&input)?;
    files::save_project(&document, &output)?;
    let reopened = files::open(&output)?;
    if document.layers() != reopened.layers() || document.metadata != reopened.metadata {
        return Err("Round-trip mismatch".into());
    }
    println!(
        "Verified {}x{}, {} layers",
        document.width,
        document.height,
        document.layers().len()
    );
    Ok(())
}
