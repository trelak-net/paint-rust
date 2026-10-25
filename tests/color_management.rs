use image::ImageEncoder;
use lcms2::{CIExyY, CIExyYTRIPLE, Profile, ToneCurve};
use paint_rust::{
    color_management::{convert_to_srgb, srgb_profile},
    files,
};
use std::sync::atomic::AtomicBool;
fn linear_rgb() -> Vec<u8> {
    let curve = ToneCurve::new(1.0);
    let xy = |x, y| CIExyY { x, y, Y: 1.0 };
    Profile::new_rgb(
        &xy(0.3127, 0.3290),
        &CIExyYTRIPLE {
            Red: xy(0.64, 0.33),
            Green: xy(0.30, 0.60),
            Blue: xy(0.15, 0.06),
        },
        &[&curve, &curve, &curve],
    )
    .unwrap()
    .icc()
    .unwrap()
}
#[test]
fn srgb_is_identity_including_hidden_rgb_and_partial_alpha() {
    let mut pixels: Vec<_> = (0..=255).map(|v| [v, 255 - v, v / 2, v]).collect();
    let original = pixels.clone();
    convert_to_srgb(
        &mut pixels,
        &srgb_profile().unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(pixels, original);
}
#[test]
fn embedded_linear_rgb_is_converted_once_on_import_and_tagged_on_export() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("linear.png");
    let mut encoder = image::codecs::png::PngEncoder::new(std::fs::File::create(&input).unwrap());
    encoder.set_icc_profile(linear_rgb()).unwrap();
    encoder
        .write_image(
            &[128, 128, 128, 37, 0, 0, 0, 255],
            2,
            1,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
    let document = files::open(&input).unwrap();
    let gray = document.pixels()[0];
    assert!((187..=189).contains(&gray[0]));
    assert!(gray[..3].iter().all(|v| (187..=189).contains(v)));
    assert_eq!(gray[3], 37);
    let output = directory.path().join("srgb.png");
    // Encoding the layer directly preserves its partially transparent straight RGB.
    let bytes = files::encode_raster_bytes(
        document.pixels(),
        (2, 1),
        files::RasterOptions::new(files::RasterFormat::Png),
        &AtomicBool::new(false),
    )
    .unwrap();
    std::fs::write(&output, bytes).unwrap();
    assert_eq!(files::open(&output).unwrap().pixels(), document.pixels());
}
#[test]
fn grayscale_profile_converts_without_changing_alpha() {
    let profile = Profile::new_gray(
        &CIExyY {
            x: 0.3457,
            y: 0.3585,
            Y: 1.0,
        },
        &ToneCurve::new(1.0),
    )
    .unwrap()
    .icc()
    .unwrap();
    let mut pixels = [[128, 128, 128, 5]];
    convert_to_srgb(&mut pixels, &profile, &AtomicBool::new(false)).unwrap();
    assert!(pixels[0][..3].iter().all(|v| (187..=189).contains(v)));
    assert_eq!(pixels[0][3], 5);
    assert!(convert_to_srgb(&mut [[128, 127, 126, 5]], &profile, &AtomicBool::new(false)).is_err());
}
#[test]
fn invalid_oversized_unsupported_and_cancelled_profiles_are_rejected() {
    for profile in [
        vec![],
        vec![0; 4 * 1024 * 1024 + 1],
        Profile::new_xyz().icc().unwrap(),
    ] {
        assert!(convert_to_srgb(&mut [[1; 4]], &profile, &AtomicBool::new(false)).is_err());
    }
    assert!(
        convert_to_srgb(
            &mut [[1; 4]],
            &srgb_profile().unwrap(),
            &AtomicBool::new(true)
        )
        .is_err()
    );
}

#[test]
fn tiny_tiff_import_reads_profile_larger_than_pixel_buffer() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("linear.tif");
    let mut encoder = image::codecs::tiff::TiffEncoder::new(std::fs::File::create(&path).unwrap());
    encoder.set_icc_profile(linear_rgb()).unwrap();
    encoder
        .write_image(&[128, 128, 128, 255], 1, 1, image::ExtendedColorType::Rgba8)
        .unwrap();
    let document = files::open(&path).unwrap();
    assert!(
        document.pixels()[0][..3]
            .iter()
            .all(|v| (187..=189).contains(v))
    );
}
