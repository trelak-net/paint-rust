use paint_rust::files::{self, RasterFormat, RasterOptions};
use std::{path::Path, sync::atomic::AtomicBool};

#[test]
fn heic_export_preview_preserves_dimensions_alpha_and_clean_state() {
    let cancel = AtomicBool::new(false);
    let pixels = vec![[85, 120, 180, 137]; 32 * 24];
    let bytes = files::encode_raster_bytes(
        &pixels,
        (32, 24),
        RasterOptions::new(RasterFormat::Heic),
        &cancel,
    )
    .unwrap();
    let doc = files::decode_raster_bytes(&bytes, RasterFormat::Heic, &cancel).unwrap();
    assert_eq!((doc.width, doc.height), (32, 24));
    for pixel in doc.pixels() {
        for c in 0..4 {
            assert!(pixel[c].abs_diff(pixels[0][c]) <= 4, "{pixel:?}");
        }
    }
    assert!(!doc.dirty());
    assert!(!doc.can_undo());
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("without-extension");
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(files::open(&path).unwrap().pixels(), doc.pixels());
    assert!(files::writable_path(Path::new("image.HEIC")));
    assert!(files::writable_path(Path::new("image.heif")));
    for length in [0, 8, bytes.len() / 2] {
        assert!(files::decode_raster_bytes(&bytes[..length], RasterFormat::Heic, &cancel).is_err());
    }
    assert_eq!(
        files::decode_raster_bytes(&bytes, RasterFormat::Heic, &AtomicBool::new(true))
            .err()
            .unwrap(),
        files::CANCELLED
    );
}

#[test]
fn heic_tiny_and_thin_images_reopen_after_codec_padding() {
    for size in [(1, 1), (1, 31), (31, 1)] {
        let pixels = vec![[10, 20, 30, 0]; size.0 as usize * size.1 as usize];
        let cancel = AtomicBool::new(false);
        let bytes = files::encode_raster_bytes(
            &pixels,
            size,
            RasterOptions::new(RasterFormat::Heic),
            &cancel,
        )
        .unwrap();
        let doc = files::decode_raster_bytes(&bytes, RasterFormat::Heic, &cancel).unwrap();
        assert_eq!((doc.width, doc.height), size);
        assert!(doc.pixels().iter().all(|pixel| pixel[3] == 0));
    }
}

fn reference_pixels() -> Vec<[u8; 4]> {
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
fn external_lossless_rgba_and_container_transform_fixtures() {
    let original = reference_pixels();
    let doc = files::open(Path::new("tests/fixtures/heic/rgba-lossless.heic")).unwrap();
    assert_eq!((doc.width, doc.height), (4, 3));
    assert_eq!(doc.pixels(), original);
    for (name, width, height) in [("rotated", 3, 4), ("mirrored", 4, 3)] {
        let doc =
            files::open(&Path::new("tests/fixtures/heic").join(format!("{name}.heic"))).unwrap();
        assert_eq!((doc.width, doc.height), (width, height));
        for y in 0..height as usize {
            for x in 0..width as usize {
                let index = if name == "rotated" {
                    (2 - x) * 4 + y
                } else {
                    y * 4 + 3 - x
                };
                assert_eq!(doc.pixels()[y * width as usize + x], original[index]);
            }
        }
    }
}

#[test]
fn external_linear_profile_is_applied_and_metadata_size_claims_are_bounded() {
    let doc = files::open(Path::new("tests/fixtures/heic/linear-icc.heic")).unwrap();
    assert_eq!((doc.width, doc.height), (2, 1));
    for channel in &doc.pixels()[0][..3] {
        assert!(channel.abs_diff(188) <= 1, "{:?}", doc.pixels());
    }
    assert_eq!(doc.pixels()[0][3], 37);
    let mut malformed = include_bytes!("fixtures/heic/rgba-lossless.heic").to_vec();
    let offset = malformed.windows(4).position(|b| b == b"ispe").unwrap();
    malformed[offset + 8..offset + 12].copy_from_slice(&500_000u32.to_be_bytes());
    assert!(
        files::decode_raster_bytes(&malformed, RasterFormat::Heic, &AtomicBool::new(false))
            .is_err()
    );
}

#[test]
fn tiled_primary_image_decodes_with_clipped_edges() {
    let doc = files::open(Path::new("tests/fixtures/heic/grid.heic")).unwrap();
    assert_eq!((doc.width, doc.height), (64, 48));
    for y in 0..48 {
        for x in 0..64 {
            assert_eq!(
                doc.pixels()[y * 64 + x],
                if x < 32 {
                    [255, 0, 0, 255]
                } else if y < 24 {
                    [0, 255, 0, 255]
                } else {
                    [0, 0, 255, 255]
                }
            );
        }
    }
}

#[test]
fn hdr_and_associated_alpha_are_explicitly_rejected() {
    for (name, expected) in [
        ("hdr-declaration", "HDR"),
        ("associated-alpha", "Associated-alpha"),
    ] {
        let error = files::open(&Path::new("tests/fixtures/heic").join(format!("{name}.heic")))
            .err()
            .unwrap();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn exported_pixels_match_independent_ffmpeg_decoder() {
    let doc = files::open(Path::new("tests/fixtures/heic/rgba-export.heic")).unwrap();
    let expected = include_bytes!("fixtures/heic/rgba-export.rgba");
    assert_eq!(doc.pixels().as_flattened().len(), expected.len());
    for (a, b) in doc.pixels().as_flattened().iter().zip(expected) {
        assert!(a.abs_diff(*b) <= 1, "{a} != {b}");
    }
}

#[test]
fn heic_density_survives_export_without_descriptive_metadata() {
    let dpi = paint_rust::resolution::Dpi {
        x: 300.25,
        y: 150.5,
    };
    let mut options = RasterOptions::new(RasterFormat::Heic);
    options.dpi = dpi;
    let cancel = AtomicBool::new(false);
    let bytes = files::encode_raster_bytes(
        &vec![[30, 60, 90, 255]; 32 * 24],
        (32, 24),
        options,
        &cancel,
    )
    .unwrap();
    let document = files::decode_raster_bytes(&bytes, RasterFormat::Heic, &cancel).unwrap();
    assert_eq!(document.resolution(), dpi);
    assert!(!document.dirty());
    let mut context = libheif_rs::HeifContext::new().unwrap();
    context.read_bytes(&bytes).unwrap();
    let handle = context.primary_image_handle().unwrap();
    assert_eq!(handle.number_of_metadata_blocks(b"Exif"), 1);
    assert_eq!(handle.number_of_metadata_blocks(b"mime"), 0);
    let mut ids = [0];
    assert_eq!(handle.metadata_block_ids(&mut ids, b"Exif"), 1);
    let exif = handle.metadata(ids[0]).unwrap();
    let start = 4 + u32::from_be_bytes(exif[..4].try_into().unwrap()) as usize;
    let tiff = &exif[start..];
    assert_eq!(&tiff[..8], b"II\x2a\0\x08\0\0\0");
    assert_eq!(u16::from_le_bytes(tiff[8..10].try_into().unwrap()), 3);
    let tags: Vec<_> = (0..3)
        .map(|i| u16::from_le_bytes(tiff[10 + i * 12..12 + i * 12].try_into().unwrap()))
        .collect();
    assert_eq!(tags, [282, 283, 296]);
    // A broken optional metadata pointer must not prevent opening the pixels.
    let mut malformed = bytes.clone();
    let offset = malformed
        .windows(exif.len())
        .position(|window| window == exif)
        .unwrap();
    malformed[offset..offset + 4].copy_from_slice(&u32::MAX.to_be_bytes());
    let recovered = files::decode_raster_bytes(&malformed, RasterFormat::Heic, &cancel).unwrap();
    assert_eq!(
        recovered.resolution(),
        paint_rust::resolution::Dpi::default()
    );
    assert_eq!(recovered.pixels(), document.pixels());
}
