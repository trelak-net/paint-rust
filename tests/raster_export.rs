use paint_rust::{
    document::{BlendMode, Document},
    files::{self, RasterFormat},
};
use std::sync::atomic::AtomicBool;

#[test]
fn png_and_webp_roundtrip_alpha_and_composite_without_changing_project() {
    let directory = tempfile::tempdir().unwrap();
    let mut doc = Document::from_pixels(
        3,
        2,
        vec![
            [10, 20, 30, 255],
            [200, 50, 10, 128],
            [0, 0, 0, 0],
            [50, 100, 150, 37],
            [100, 200, 50, 200],
            [255, 255, 255, 255],
        ],
    )
    .unwrap();
    doc.add_layer(true);
    doc.set_layer_style("Overlay".into(), 90, BlendMode::Screen);
    let composite = doc.composite();
    let history = doc.history().count();
    for format in [RasterFormat::Png, RasterFormat::WebP] {
        let path = directory
            .path()
            .join(format!("export.{}", format.extension()));
        files::save_raster_cancellable(&doc, &path, format, &AtomicBool::new(false)).unwrap();
        let loaded = files::open(&path).unwrap();
        assert_eq!((loaded.width, loaded.height), (3, 2));
        assert_eq!(loaded.pixels(), composite);
        assert!(doc.dirty());
        assert_eq!(doc.history().count(), history);
        assert_eq!(doc.layers().len(), 2);
    }
}

#[test]
fn cancelled_webp_export_does_not_replace_destination() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("export.webp");
    std::fs::write(&path, b"existing image").unwrap();
    let doc = Document::new(4, 4, [255; 4]).unwrap();
    assert!(
        files::save_raster_cancellable(&doc, &path, RasterFormat::WebP, &AtomicBool::new(true))
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"existing image");
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}
