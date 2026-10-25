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

#[test]
fn opaque_formats_apply_the_selected_matte_and_preserve_preview_bytes_on_save() {
    use files::RasterOptions;
    let cancel = AtomicBool::new(false);
    let source = [[200, 40, 80, 0], [200, 40, 80, 128], [200, 40, 80, 255]];
    let options = RasterOptions {
        format: RasterFormat::Bmp,
        quality: 90,
        matte: [20, 100, 180],
    };
    let bytes = files::encode_raster_bytes(&source, (3, 1), options, &cancel).unwrap();
    let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(
        decoded.as_raw(),
        &[20, 100, 180, 255, 110, 70, 130, 255, 200, 40, 80, 255]
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("output.bmp");
    files::save_encoded_raster(&bytes, &path, &cancel).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(files::save_encoded_raster(b"replacement", &path, &AtomicBool::new(true)).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(source[0], [200, 40, 80, 0]);
}

#[test]
fn jpeg_export_has_valid_dimensions_quality_control_and_opaque_matte() {
    use files::RasterOptions;
    let cancel = AtomicBool::new(false);
    for quality in [1, 50, 100] {
        let bytes = files::encode_raster_bytes(
            &vec![[255, 0, 0, 0]; 32 * 16],
            (32, 16),
            RasterOptions {
                format: RasterFormat::Jpeg,
                quality,
                matte: [128; 3],
            },
            &cancel,
        )
        .unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (32, 16));
        assert!(
            decoded
                .pixels()
                .all(|p| p[3] == 255 && p.0[..3].iter().all(|v| v.abs_diff(128) <= 3))
        );
    }
    assert!(
        files::encode_raster_bytes(
            &[[0; 4]],
            (1, 1),
            RasterOptions {
                quality: 0,
                ..RasterOptions::new(RasterFormat::Jpeg)
            },
            &cancel
        )
        .is_err()
    );
    assert!(
        files::encode_raster_bytes(&[], (1, 1), RasterOptions::new(RasterFormat::Bmp), &cancel)
            .is_err()
    );
    assert!(
        files::encode_raster_bytes(
            &[[0; 4]],
            (1, 1),
            RasterOptions::new(RasterFormat::Png),
            &AtomicBool::new(true)
        )
        .is_err()
    );
}

#[test]
fn raster_export_omits_private_metadata_and_tags_supported_formats_as_srgb() {
    use image::{ImageDecoder, ImageReader, metadata::Orientation};
    let directory = tempfile::tempdir().unwrap();
    let mut document = Document::new(3, 2, [80, 120, 160, 255]).unwrap();
    let marker = "private-export-metadata-marker-93c1";
    for key in ["Author", "GPS", "Software", "Comment", "Orientation"] {
        document.metadata.insert(key.into(), marker.into());
    }
    for format in [
        RasterFormat::Png,
        RasterFormat::Jpeg,
        RasterFormat::Bmp,
        RasterFormat::WebP,
        RasterFormat::Tiff,
        RasterFormat::Tga,
        RasterFormat::Gif,
    ] {
        let path = directory
            .path()
            .join(format!("image.{}", format.extension()));
        files::save_raster_cancellable(&document, &path, format, &AtomicBool::new(false)).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            !bytes
                .windows(marker.len())
                .any(|window| window == marker.as_bytes())
        );
        let mut decoder = ImageReader::open(&path).unwrap().into_decoder().unwrap();
        assert!(decoder.exif_metadata().unwrap().is_none());
        let profile = if format == RasterFormat::Tiff {
            let mut tiff = tiff::decoder::Decoder::new(std::io::Cursor::new(&bytes)).unwrap();
            Some(tiff.get_tag_u8_vec(tiff::tags::Tag::IccProfile).unwrap())
        } else {
            decoder.icc_profile().unwrap()
        };
        if matches!(
            format,
            RasterFormat::Bmp | RasterFormat::Tga | RasterFormat::Gif
        ) {
            assert!(profile.is_none());
        } else {
            assert_eq!(
                profile.unwrap_or_else(|| panic!("Missing ICC profile for {format:?}")),
                paint_rust::color_management::srgb_profile().unwrap()
            );
        }
        assert_eq!(decoder.orientation().unwrap(), Orientation::NoTransforms);
        assert_eq!(document.metadata.get("Author").unwrap(), marker);
    }
}

#[test]
fn tiff_and_tga_preserve_rgba_and_atomic_output_bytes() {
    let pixels = [
        [19, 87, 201, 0],
        [31, 57, 193, 1],
        [211, 19, 97, 128],
        [4, 80, 125, 255],
    ];
    let directory = tempfile::tempdir().unwrap();
    for format in [RasterFormat::Tiff, RasterFormat::Tga] {
        let bytes = files::encode_raster_bytes(
            &pixels,
            (2, 2),
            files::RasterOptions::new(format),
            &AtomicBool::new(false),
        )
        .unwrap();
        let path = directory
            .path()
            .join(format!("exact.{}", format.extension()));
        files::save_encoded_raster(&bytes, &path, &AtomicBool::new(false)).unwrap();
        let loaded = files::open(&path).unwrap();
        assert_eq!((loaded.width, loaded.height), (2, 2));
        assert_eq!(loaded.pixels(), pixels);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(!loaded.dirty());
        assert!(files::open_cancellable(&path, &AtomicBool::new(true)).is_err());
        assert!(files::save_encoded_raster(b"wrong", &path, &AtomicBool::new(true)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}
