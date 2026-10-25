use paint_rust::files;
use std::{path::Path, sync::atomic::AtomicBool};
fn pixels() -> Vec<[u8; 4]> {
    vec![
        [0, 0, 0, 0],
        [255, 0, 0, 64],
        [0, 255, 0, 128],
        [0, 0, 255, 255],
        [30, 60, 90, 255],
        [200, 150, 100, 200],
        [255, 255, 255, 255],
        [0, 0, 0, 255],
        [12, 34, 56, 255],
        [90, 80, 70, 1],
        [120, 130, 140, 254],
        [240, 230, 220, 255],
    ]
}
#[test]
fn independent_lossless_rgba_fixture_preserves_every_pixel() {
    let document = files::open(Path::new("tests/fixtures/jxl/rgba-lossless.jxl")).unwrap();
    assert_eq!((document.width, document.height), (4, 3));
    assert_eq!(document.pixels(), pixels());
    assert!(!document.dirty());
    assert!(!document.can_undo());
}
#[test]
fn codestream_orientation_is_applied_exactly_once() {
    let document = files::open(Path::new("tests/fixtures/jxl/oriented-lossless.jxl")).unwrap();
    assert_eq!((document.width, document.height), (3, 4));
    let original = pixels();
    for y in 0..4 {
        for x in 0..3 {
            assert_eq!(document.pixels()[y * 3 + x], original[(2 - x) * 4 + y]);
        }
    }
}
#[test]
fn sixteen_bit_grayscale_import_converts_to_rgba8() {
    let document = files::open(Path::new("tests/fixtures/jxl/gray16-lossless.jxl")).unwrap();
    assert_eq!((document.width, document.height), (5, 3));
    for y in 0..3 {
        for x in 0..5 {
            assert_eq!(
                document.pixels()[y * 5 + x],
                [[0, 0, 0, 255], [128, 128, 128, 255], [255; 4]][y]
            );
        }
    }
}
#[test]
fn signatures_work_without_extensions_and_bad_or_cancelled_inputs_fail() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("image");
    let bytes = include_bytes!("fixtures/jxl/rgba-lossless.jxl");
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(files::open(&path).unwrap().pixels(), pixels());
    assert_eq!(
        files::open_cancellable(&path, &AtomicBool::new(true))
            .err()
            .unwrap(),
        files::CANCELLED
    );
    for data in [&bytes[..8], &bytes[..bytes.len() / 2], b"not an image"] {
        let path = root.path().join("broken.jxl");
        std::fs::write(&path, data).unwrap();
        assert!(files::open(&path).is_err());
    }
}

#[test]
fn import_only_formats_require_a_new_save_destination() {
    for path in ["image.jxl", "image.JXL", "image.unknown"] {
        assert!(!files::writable_path(Path::new(path)));
    }
    for path in [
        "image.PDN",
        "image.prust",
        "image.PNG",
        "image.gif",
        "image.dds",
    ] {
        assert!(files::writable_path(Path::new(path)));
    }
}

#[test]
fn embedded_linear_profile_is_converted_before_rgba8_quantization() {
    let document = files::open(Path::new("tests/fixtures/jxl/linear-icc.jxl")).unwrap();
    assert_eq!((document.width, document.height), (2, 1));
    let gray = document.pixels()[0];
    assert!(
        gray[..3].iter().all(|value| (187..=189).contains(value)),
        "{gray:?}"
    );
    assert_eq!(gray[3], 37);
    assert_eq!(document.pixels()[1][3], 255);
}

#[test]
fn animation_import_uses_only_the_first_displayed_frame() {
    let decoded = jxl_oxide::JxlImage::builder()
        .open("tests/fixtures/jxl/animation.jxl")
        .unwrap();
    assert!(decoded.image_header().metadata.animation.is_some());
    assert_eq!(decoded.num_loaded_keyframes(), 2);
    let document = files::open(Path::new("tests/fixtures/jxl/animation.jxl")).unwrap();
    assert_eq!((document.width, document.height), (2, 2));
    assert_eq!(document.pixels(), [[255, 0, 0, 255]; 4]);
}

#[test]
fn container_signature_is_detected_and_oversized_dimensions_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("container-without-extension");
    std::fs::write(&path, include_bytes!("fixtures/jxl/rgba-container.jxl")).unwrap();
    assert_eq!(files::open(&path).unwrap().pixels(), pixels());
    assert!(files::open(Path::new("tests/fixtures/jxl/oversized-width.jxl")).is_err());
}
