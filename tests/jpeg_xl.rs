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
fn unsupported_output_extensions_require_a_new_save_destination() {
    assert!(!files::writable_path(Path::new("image.unknown")));
    for path in [
        "image.PDN",
        "image.prust",
        "image.PNG",
        "image.jxl",
        "image.JXL",
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

#[test]
fn lossless_export_preserves_straight_rgba_including_hidden_rgb() {
    use files::{RasterFormat, RasterOptions};
    let cancel = AtomicBool::new(false);
    for size in [(1, 1), (1, 257), (257, 1), (4, 3), (37, 19), (257, 257)] {
        let pixels: Vec<_> = (0..size.0 * size.1)
            .map(|i| {
                [
                    (i * 37) as u8,
                    (i * 91) as u8,
                    (i * 17 + 200) as u8,
                    (i * 51) as u8,
                ]
            })
            .collect();
        let bytes = files::encode_raster_bytes(
            &pixels,
            size,
            RasterOptions::new(RasterFormat::JpegXl),
            &cancel,
        )
        .unwrap();
        let decoded = files::decode_raster_bytes(&bytes, RasterFormat::JpegXl, &cancel).unwrap();
        assert_eq!((decoded.width, decoded.height), size);
        assert_eq!(decoded.pixels(), pixels);
        assert!(
            files::encode_raster_bytes(
                &pixels,
                size,
                RasterOptions::new(RasterFormat::JpegXl),
                &AtomicBool::new(true)
            )
            .is_err()
        );
    }
}

#[test]
fn independently_decoded_exports_remain_exact() {
    let document = files::open(Path::new("tests/fixtures/jxl/rgba-export.jxl")).unwrap();
    assert_eq!(document.pixels(), pixels());
    let thin = files::open(Path::new("tests/fixtures/jxl/thin-export.jxl")).unwrap();
    assert_eq!((thin.width, thin.height), (1, 257));
    for (i, pixel) in thin.pixels().iter().enumerate() {
        assert_eq!(
            *pixel,
            [
                (i * 37) as u8,
                (i * 91) as u8,
                (i * 17 + 200) as u8,
                (i * 51) as u8
            ]
        );
    }
}

fn container_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    bytes.extend(kind);
    bytes.extend(payload);
    bytes
}
#[test]
fn trailing_density_metadata_is_read_and_private_xml_is_not_reexported() {
    use files::{RasterFormat, RasterOptions};
    let dpi = paint_rust::resolution::Dpi {
        x: 300.125,
        y: 150.75,
    };
    let cancel = AtomicBool::new(false);
    let output = files::encode_raster_bytes(
        &pixels(),
        (4, 3),
        RasterOptions {
            dpi,
            ..RasterOptions::new(RasterFormat::JpegXl)
        },
        &cancel,
    )
    .unwrap();
    let exif_length = u32::from_be_bytes(output[32..36].try_into().unwrap()) as usize;
    let mut reordered = output[..32].to_vec();
    reordered.extend(&output[32 + exif_length..]);
    reordered.extend(container_box(b"free", &vec![0; 5000]));
    reordered.extend(container_box(b"xml ", b"private-user-name"));
    reordered.extend(&output[32..32 + exif_length]);
    let document = files::decode_raster_bytes(&reordered, RasterFormat::JpegXl, &cancel).unwrap();
    assert_eq!(document.resolution(), dpi);
    assert_eq!(document.pixels(), pixels());
    assert!(!document.dirty());
    let clean = files::encode_raster_bytes(
        document.pixels(),
        (4, 3),
        RasterOptions {
            dpi: document.resolution(),
            ..RasterOptions::new(RasterFormat::JpegXl)
        },
        &cancel,
    )
    .unwrap();
    assert!(!clean.windows(17).any(|bytes| bytes == b"private-user-name"));
}
#[test]
fn codestream_quarter_turn_swaps_density_axes_without_exif_rotation() {
    let dpi = paint_rust::resolution::Dpi { x: 300.0, y: 150.0 };
    let mut bytes = b"\0\0\0\x0cJXL \r\n\x87\n\0\0\0\x14ftypjxl \0\0\0\0jxl ".to_vec();
    let mut exif = vec![0; 4];
    exif.extend(paint_rust::raster_density::exif(dpi));
    bytes.extend(container_box(b"Exif", &exif));
    bytes.extend(container_box(
        b"jxlc",
        include_bytes!("fixtures/jxl/oriented-lossless.jxl"),
    ));
    let document =
        files::decode_raster_bytes(&bytes, files::RasterFormat::JpegXl, &AtomicBool::new(false))
            .unwrap();
    assert_eq!((document.width, document.height), (3, 4));
    assert_eq!(
        document.resolution(),
        paint_rust::resolution::Dpi { x: 150.0, y: 300.0 }
    );
}

#[test]
fn externally_verified_density_export_is_clean_on_import() {
    let document = files::open(Path::new("tests/fixtures/jxl/resolution-export.jxl")).unwrap();
    assert_eq!((document.width, document.height), (300, 150));
    assert_eq!(
        document.resolution(),
        paint_rust::resolution::Dpi { x: 300.0, y: 300.0 }
    );
    assert!(!document.dirty());
    assert!(
        document
            .pixels()
            .iter()
            .all(|pixel| *pixel == [40, 80, 120, 255])
    );
}
