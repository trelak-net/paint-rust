use paint_rust::files::{self, RasterFormat, RasterOptions};
use std::sync::atomic::AtomicBool;
#[test]
fn single_frame_export_has_binary_alpha_and_selected_matte() {
    let pixels = [
        [200, 40, 80, 0],
        [200, 40, 80, 127],
        [200, 40, 80, 128],
        [200, 40, 80, 255],
    ];
    let options = RasterOptions {
        format: RasterFormat::Gif,
        quality: 90,
        gif_dither: false,
        dds_mipmaps: false,
        dds_compression: Default::default(),
        dpi: Default::default(),
        matte: [20, 60, 100],
    };
    let bytes =
        files::encode_raster_bytes(&pixels, (4, 1), options, &AtomicBool::new(false)).unwrap();
    assert!(bytes.starts_with(b"GIF89a"));
    let decoded =
        files::decode_raster_bytes(&bytes, RasterFormat::Gif, &AtomicBool::new(false)).unwrap();
    assert_eq!(decoded.pixels()[0][3], 0);
    assert_eq!(decoded.pixels()[1][3], 0);
    assert_eq!(decoded.pixels()[2], [110, 50, 90, 255]);
    assert_eq!(decoded.pixels()[3], pixels[3]);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.gif");
    files::save_encoded_raster(&bytes, &path, &AtomicBool::new(false)).unwrap();
    assert_eq!(files::open(&path).unwrap().pixels(), decoded.pixels());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
#[test]
fn animation_import_uses_first_frame() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("animation.gif");
    {
        let mut encoder =
            image::codecs::gif::GifEncoder::new(std::fs::File::create(&path).unwrap());
        for pixel in [[255, 0, 0, 255], [0, 0, 255, 255]] {
            encoder
                .encode_frame(image::Frame::new(image::RgbaImage::from_pixel(
                    2,
                    2,
                    image::Rgba(pixel),
                )))
                .unwrap();
        }
    }
    let document = files::open(&path).unwrap();
    assert_eq!(document.pixels(), &[[255, 0, 0, 255]; 4]);
    assert_eq!(document.layers().len(), 1);
}
#[test]
fn gif_cancellation_preserves_existing_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.gif");
    std::fs::write(&path, b"old").unwrap();
    let document = paint_rust::document::Document::new(1, 1, [255; 4]).unwrap();
    assert!(
        files::save_raster_cancellable(&document, &path, RasterFormat::Gif, &AtomicBool::new(true))
            .is_err()
    );
    assert_eq!(std::fs::read(path).unwrap(), b"old");
}

#[test]
fn dithering_changes_quantized_gradients_and_keeps_binary_transparency() {
    let size = (128, 96);
    let pixels: Vec<_> = (0..size.0 * size.1)
        .map(|i| {
            let (x, y) = (i % size.0, i / size.0);
            [
                (x * 2) as u8,
                (y * 255 / 95) as u8,
                ((x + y) * 255 / 222) as u8,
                if x < 3 { 0 } else { 255 },
            ]
        })
        .collect();
    let cancel = AtomicBool::new(false);
    let plain = files::encode_raster_bytes(
        &pixels,
        size,
        RasterOptions::new(RasterFormat::Gif),
        &cancel,
    )
    .unwrap();
    let options = RasterOptions {
        gif_dither: true,
        ..RasterOptions::new(RasterFormat::Gif)
    };
    let dithered = files::encode_raster_bytes(&pixels, size, options, &cancel).unwrap();
    assert_ne!(plain, dithered);
    assert_eq!(
        dithered,
        files::encode_raster_bytes(&pixels, size, options, &cancel).unwrap()
    );
    let decoded = files::decode_raster_bytes(&dithered, RasterFormat::Gif, &cancel).unwrap();
    for (actual, source) in decoded.pixels().iter().zip(&pixels) {
        assert_eq!(actual[3], source[3]);
    }
    let colors: std::collections::HashSet<_> = decoded.pixels().iter().collect();
    assert!(colors.len() <= 256);
    assert!(files::encode_raster_bytes(&pixels, size, options, &AtomicBool::new(true)).is_err());
}

#[test]
fn dithering_preserves_exact_small_palettes_and_transparent_only_images() {
    let cancel = AtomicBool::new(false);
    for pixels in [
        vec![[0; 4]; 6],
        vec![
            [1, 2, 3, 255],
            [1, 2, 4, 255],
            [255, 255, 255, 255],
            [1, 2, 3, 255],
            [1, 2, 4, 255],
            [0; 4],
        ],
    ] {
        let options = RasterOptions {
            gif_dither: true,
            ..RasterOptions::new(RasterFormat::Gif)
        };
        let bytes = files::encode_raster_bytes(&pixels, (3, 2), options, &cancel).unwrap();
        let decoded = files::decode_raster_bytes(&bytes, RasterFormat::Gif, &cancel).unwrap();
        assert_eq!(decoded.pixels(), pixels);
    }
}
