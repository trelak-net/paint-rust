//! Isolated child for scripts/save_failure_smoke.py; it never opens a user file.
use paint_rust::{document::Document, files};
use std::{path::PathBuf, sync::atomic::AtomicBool};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let path = PathBuf::from(&args[1]);
    let format = &args[2];
    let pixels: Vec<_> = (0..4096u32)
        .map(|i| [(i * 17) as u8, (i * 31) as u8, (i >> 3) as u8, 255])
        .collect();
    let doc = Document::from_pixels(64, 64, pixels.clone()).unwrap();
    let cancel = AtomicBool::new(false);
    let result = match format.as_str() {
        "prust" | "pdn" => files::save_project_cancellable(&doc, &path, &cancel),
        "png" => files::save_png_cancellable(&doc, &path, &cancel),
        "encoded" => files::save_encoded_raster(&vec![42; 16384], &path, &cancel),
        _ => panic!("Unknown fixture format"),
    };
    let error = result.expect_err("Injected fault did not reach the save operation");
    assert_eq!(doc.pixels(), pixels);
    assert!(!doc.dirty());
    assert!(!doc.can_undo());
    println!("{error}");
}
