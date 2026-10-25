use paint_rust::files::{self, RasterFormat, RasterOptions};
use std::{path::Path, sync::atomic::AtomicBool};

#[test]
fn avif_quality_preview_reopens_with_alpha_and_clean_state() {
    let pixels = vec![[85, 120, 180, 137]; 32 * 24];
    let cancel = AtomicBool::new(false);
    let bytes = files::encode_raster_bytes(
        &pixels,
        (32, 24),
        RasterOptions::new(RasterFormat::Avif),
        &cancel,
    )
    .unwrap();
    let doc = files::decode_raster_bytes(&bytes, RasterFormat::Avif, &cancel).unwrap();
    assert_eq!((doc.width, doc.height), (32, 24));
    for p in doc.pixels() {
        for c in 0..4 {
            assert!(p[c].abs_diff(pixels[0][c]) <= 3, "{p:?}");
        }
    }
    assert!(!doc.dirty());
    assert!(!doc.can_undo());
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("without-extension");
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(files::open(&path).unwrap().pixels(), doc.pixels());
    assert!(files::writable_path(Path::new("image.AVIF")));
    for length in [0, 8, bytes.len() / 2] {
        assert!(files::decode_raster_bytes(&bytes[..length], RasterFormat::Avif, &cancel).is_err());
    }
    assert_eq!(
        files::decode_raster_bytes(&bytes, RasterFormat::Avif, &AtomicBool::new(true))
            .err()
            .unwrap(),
        files::CANCELLED
    );
}

#[test]
fn avif_handles_single_pixel_dimensions_and_transparency() {
    for size in [(1, 1), (1, 31), (31, 1)] {
        let pixels = vec![[10, 20, 30, 0]; size.0 as usize * size.1 as usize];
        let cancel = AtomicBool::new(false);
        let bytes = files::encode_raster_bytes(
            &pixels,
            size,
            RasterOptions::new(RasterFormat::Avif),
            &cancel,
        )
        .unwrap();
        let doc = files::decode_raster_bytes(&bytes, RasterFormat::Avif, &cancel).unwrap();
        assert_eq!((doc.width, doc.height), size);
        assert!(doc.pixels().iter().all(|p| p[3] == 0));
    }
}

fn reference_pixels() -> Vec<[u8; 4]> {
    (0..6)
        .flat_map(|y| {
            (0..8).map(move |x| {
                if x < 3 {
                    [255, 0, 0, 255]
                } else if y < 3 {
                    [0, 255, 0, 255]
                } else {
                    [0, 0, 255, 255]
                }
            })
        })
        .collect()
}

#[test]
fn independent_libaom_fixture_decodes_exactly_and_transforms_once() {
    let original = reference_pixels();
    let doc = files::open(Path::new("tests/fixtures/avif/rgb-lossless.avif")).unwrap();
    assert_eq!((doc.width, doc.height), (8, 6));
    assert_eq!(doc.pixels(), original);
    for (name, width, height) in [
        ("rotated", 6, 8),
        ("mirrored", 8, 6),
        ("rotated-mirrored", 6, 8),
    ] {
        let doc =
            files::open(&Path::new("tests/fixtures/avif").join(format!("{name}.avif"))).unwrap();
        assert_eq!((doc.width, doc.height), (width, height));
        for y in 0..height as usize {
            for x in 0..width as usize {
                let index = match name {
                    "rotated" => x * 8 + 7 - y,
                    "mirrored" => y * 8 + 7 - x,
                    _ => x * 8 + y,
                };
                assert_eq!(
                    doc.pixels()[y * width as usize + x],
                    original[index],
                    "{name}: {x},{y}"
                );
            }
        }
    }
}

#[test]
fn oversized_hdr_and_conflicting_container_color_declarations_are_rejected() {
    for (name, message) in [("oversized-width", "16,384"), ("hdr-declaration", "HDR")] {
        let error = files::open(&Path::new("tests/fixtures/avif").join(format!("{name}.avif")))
            .err()
            .unwrap();
        assert!(error.contains(message), "{error}");
    }
    assert!(files::open(Path::new("tests/fixtures/avif/oversized-area.avif")).is_err());
    let mut bytes = include_bytes!("fixtures/avif/rgb-lossless.avif").to_vec();
    let offset = bytes.windows(4).position(|b| b == b"nclx").unwrap();
    bytes[offset + 9] = 1; // Container claims BT.709 matrix, coded frame uses RGB identity.
    let error = files::decode_raster_bytes(&bytes, RasterFormat::Avif, &AtomicBool::new(false))
        .err()
        .unwrap();
    assert!(error.contains("color overrides"), "{error}");
}

#[test]
fn exported_rgba_matches_independent_libaom_decoding() {
    let doc = files::open(Path::new("tests/fixtures/avif/rgba-export.avif")).unwrap();
    assert_eq!((doc.width, doc.height), (4, 3));
    assert_eq!(
        doc.pixels().as_flattened(),
        include_bytes!("fixtures/avif/rgba-export.rgba")
    );
}

#[test]
fn embedded_linear_rgb_profile_is_converted_to_srgb() {
    let doc = files::open(Path::new("tests/fixtures/avif/linear-icc.avif")).unwrap();
    assert_eq!((doc.width, doc.height), (2, 1));
    for channel in &doc.pixels()[0][..3] {
        assert!(channel.abs_diff(188) <= 1, "{:?}", doc.pixels());
    }
    assert_eq!(doc.pixels()[0][3], 255);
}
