//! Generate original color/alpha fixtures for independent DDS decoder checks.
use paint_rust::{
    dds::Compression,
    files::{self, RasterFormat, RasterOptions},
};
use std::{path::PathBuf, sync::atomic::AtomicBool};
fn main() {
    let folder = PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&folder).unwrap();
    let pixels: Vec<_> = (0..64)
        .flat_map(|y| {
            (0..64).map(move |x| {
                [
                    (x * 4) as u8,
                    (y * 4) as u8,
                    ((x + y) * 2) as u8,
                    ((x * 17 + y * 31) % 256) as u8,
                ]
            })
        })
        .collect();
    for (name, compression) in [
        ("export-bc1", Compression::Bc1),
        ("export-bc3", Compression::Bc3),
        ("export-bc4", Compression::Bc4),
        ("export-bc5", Compression::Bc5),
    ] {
        let bytes = files::encode_raster_bytes(
            &pixels,
            (64, 64),
            RasterOptions {
                dds_compression: compression,
                dds_mipmaps: true,
                ..RasterOptions::new(RasterFormat::Dds)
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        std::fs::write(folder.join(format!("{name}.dds")), bytes).unwrap();
    }
}
