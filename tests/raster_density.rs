use paint_rust::{
    document::Document,
    files::{self, RasterFormat, RasterOptions},
    raster_density,
    resolution::Dpi,
};
use std::{io::Cursor, sync::atomic::AtomicBool};

#[test]
fn supported_rasters_preserve_independent_fractional_density_without_dirtying_import() {
    let cancel = AtomicBool::new(false);
    let dir = tempfile::tempdir().unwrap();
    let dpi = Dpi {
        x: 300.125,
        y: 150.75,
    };
    let mut doc = Document::from_pixels(2, 1, vec![[120, 80, 40, 128], [20, 30, 40, 255]]).unwrap();
    doc.set_resolution(dpi).unwrap();
    for format in [
        RasterFormat::Png,
        RasterFormat::Jpeg,
        RasterFormat::Bmp,
        RasterFormat::WebP,
        RasterFormat::Tiff,
        RasterFormat::JpegXl,
    ] {
        let path = dir.path().join(format!("density.{}", format.extension()));
        files::save_raster_cancellable(&doc, &path, format, &cancel).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        for loaded in [
            files::open(&path).unwrap(),
            files::decode_raster_bytes(&bytes, format, &cancel).unwrap(),
        ] {
            let actual = loaded.resolution();
            assert!((actual.x - dpi.x).abs() < 0.013, "{format:?}: {actual:?}");
            assert!((actual.y - dpi.y).abs() < 0.013, "{format:?}: {actual:?}");
            assert!(!loaded.dirty());
        }
        if format == RasterFormat::Tiff {
            let mut decoder = tiff::decoder::Decoder::new(Cursor::new(&bytes)).unwrap();
            assert_eq!(
                decoder
                    .get_tag_u16_vec(tiff::tags::Tag::ExtraSamples)
                    .unwrap(),
                vec![2]
            );
            assert_eq!(files::open(&path).unwrap().pixels(), doc.pixels());
        }
    }
}

#[test]
fn malformed_or_unitless_exif_does_not_supply_density() {
    let dpi = Dpi { x: 300.0, y: 150.0 };
    let exif = raster_density::exif(dpi);
    assert_eq!(raster_density::read_exif(&exif), Some(dpi));
    for end in 0..exif.len() {
        assert!(raster_density::read_exif(&exif[..end]).is_none());
    }
    let mut malformed = exif.clone();
    malformed[54..58].fill(0);
    assert!(raster_density::read_exif(&malformed).is_none());
    let mut unitless = exif.clone();
    unitless[42..44].copy_from_slice(&1u16.to_le_bytes());
    assert!(raster_density::read_exif(&unitless).is_none());
    let mut offset = exif;
    offset[18..22].fill(255);
    assert!(raster_density::read_exif(&offset).is_none());
}

#[test]
fn invalid_export_density_is_rejected() {
    for dpi in [
        Dpi {
            x: f64::NAN,
            y: 96.0,
        },
        Dpi { x: 0.0, y: 96.0 },
        Dpi {
            x: 96.0,
            y: 10001.0,
        },
    ] {
        assert!(
            files::encode_raster_bytes(
                &[[255; 4]],
                (1, 1),
                RasterOptions {
                    dpi,
                    ..RasterOptions::new(RasterFormat::Png)
                },
                &AtomicBool::new(false)
            )
            .is_err()
        );
    }
}
