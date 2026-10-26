use paint_rust::{
    dds,
    files::{self, RasterFormat, RasterOptions},
};
use std::{io::Cursor, sync::atomic::AtomicBool};
fn cancel() -> AtomicBool {
    AtomicBool::new(false)
}
fn set(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    dds::encode(
        &vec![[0; 4]; (width * height) as usize],
        (width, height),
        &mut bytes,
        &cancel(),
    )
    .unwrap();
    bytes.truncate(128);
    bytes
}
#[test]
fn rgba_export_preserves_exact_channels_including_transparent_rgb() {
    let pixels = [
        [10, 20, 30, 0],
        [40, 50, 60, 1],
        [70, 80, 90, 128],
        [100, 110, 120, 255],
        [1, 2, 3, 250],
        [255; 4],
    ];
    let bytes = files::encode_raster_bytes(
        &pixels,
        (3, 2),
        RasterOptions::new(RasterFormat::Dds),
        &cancel(),
    )
    .unwrap();
    assert_eq!(&bytes[..4], b"DDS ");
    assert_eq!(bytes.len(), 128 + 24);
    let doc = files::decode_raster_bytes(&bytes, RasterFormat::Dds, &cancel()).unwrap();
    assert_eq!(doc.pixels(), pixels);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("texture.dds");
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(files::open(&path).unwrap().pixels(), pixels);
}
#[test]
fn bgr24_pitch_and_rgb565_channels_are_decoded() {
    let mut bytes = header(1, 2);
    set(&mut bytes, 80, 0x40);
    set(&mut bytes, 88, 24);
    set(&mut bytes, 20, 4);
    set(&mut bytes, 92, 0xff0000);
    set(&mut bytes, 100, 0xff);
    bytes.extend([30, 20, 10, 99, 60, 50, 40, 99]);
    assert_eq!(
        dds::decode(Cursor::new(bytes), &cancel()).unwrap().pixels(),
        &[[10, 20, 30, 255], [40, 50, 60, 255]]
    );
    let mut bytes = header(2, 1);
    set(&mut bytes, 80, 0x40);
    set(&mut bytes, 88, 16);
    set(&mut bytes, 20, 4);
    set(&mut bytes, 92, 0xf800);
    set(&mut bytes, 96, 0x07e0);
    set(&mut bytes, 100, 0x1f);
    bytes.extend([0, 248, 224, 7]);
    assert_eq!(
        dds::decode(Cursor::new(bytes), &cancel()).unwrap().pixels(),
        &[[255, 0, 0, 255], [0, 255, 0, 255]]
    );
}
fn compressed(kind: &[u8; 4], size: (u32, u32), blocks: &[u8]) -> Vec<u8> {
    let mut bytes = header(size.0, size.1);
    set(&mut bytes, 80, 4);
    bytes[84..88].copy_from_slice(kind);
    bytes.extend(blocks);
    bytes
}
#[test]
fn bc1_transparency_and_partial_edge_blocks_are_preserved() {
    let bytes = compressed(b"DXT1", (3, 1), &[0, 0, 255, 255, 0b00111001, 0, 0, 0]);
    let doc = dds::decode(Cursor::new(bytes), &cancel()).unwrap();
    assert_eq!(doc.pixels(), &[[255; 4], [127, 127, 127, 255], [0; 4]]);
}
#[test]
fn bc2_and_bc3_alpha_use_their_respective_index_layouts() {
    let bytes = compressed(
        b"DXT3",
        (2, 1),
        &[0xf0, 0, 0, 0, 0, 0, 0, 0, 0, 248, 0, 0, 0, 0, 0, 0],
    );
    assert_eq!(
        dds::decode(Cursor::new(bytes), &cancel()).unwrap().pixels(),
        &[[255, 0, 0, 0], [255, 0, 0, 255]]
    );
    for (a, b, expected) in [
        (255, 0, [255, 0, 218, 182, 145, 109, 72, 36]),
        (0, 255, [0, 255, 51, 102, 153, 204, 0, 255]),
    ] {
        let mut indices = 0u64;
        for i in 0..16 {
            indices |= (i % 8) << (i * 3);
        }
        let mut block = vec![a, b];
        block.extend_from_slice(&indices.to_le_bytes()[..6]);
        block.extend([0, 248, 0, 0, 0, 0, 0, 0]);
        let doc = dds::decode(Cursor::new(compressed(b"DXT5", (4, 4), &block)), &cancel()).unwrap();
        for (i, pixel) in doc.pixels().iter().enumerate() {
            assert_eq!(*pixel, [255, 0, 0, expected[i % 8]]);
        }
    }
}
#[test]
fn malformed_unsupported_truncated_and_cancelled_files_fail_cleanly() {
    let good = header(2, 2);
    for (offset, value) in [
        (4, 123),
        (76, 31),
        (16, u32::MAX),
        (112, 0x200),
        (20, 1),
        (92, 0xff00),
        (88, 64),
    ] {
        let mut bytes = good.clone();
        set(&mut bytes, offset, value);
        bytes.extend([0; 16]);
        assert!(dds::decode(Cursor::new(bytes), &cancel()).is_err());
    }
    for length in [0, 4, 127, 128] {
        assert!(dds::decode(Cursor::new(&good[..length]), &cancel()).is_err());
    }
    assert!(
        dds::decode(
            Cursor::new(compressed(b"BC7 ", (4, 4), &[0; 16])),
            &cancel()
        )
        .is_err()
    );
    assert!(dds::decode(Cursor::new(good), &AtomicBool::new(true)).is_err());
}

#[test]
fn independently_encoded_block_fixtures_match_external_decoder() {
    for (encoded, expected) in [
        (
            include_bytes!("fixtures/dds/gradient-bc1.dds").as_slice(),
            include_bytes!("fixtures/dds/gradient-bc1.rgba").as_slice(),
        ),
        (
            include_bytes!("fixtures/dds/gradient-bc3.dds").as_slice(),
            include_bytes!("fixtures/dds/gradient-bc3.rgba").as_slice(),
        ),
    ] {
        let document = dds::decode(Cursor::new(encoded), &cancel()).unwrap();
        assert_eq!((document.width, document.height), (8, 8));
        for (pixel, reference) in document
            .pixels()
            .iter()
            .zip(expected.as_chunks::<4>().0.iter())
        {
            assert_eq!(pixel[3], reference[3]);
            for channel in 0..3 {
                assert!(
                    pixel[channel].abs_diff(reference[channel]) <= 1,
                    "{pixel:?} != {reference:?}"
                );
            }
        }
    }
}

#[test]
fn dx10_rgba_and_opaque_bc3_are_supported_but_arrays_are_rejected() {
    let mut bytes = header(1, 1);
    set(&mut bytes, 80, 4);
    bytes[84..88].copy_from_slice(b"DX10");
    for value in [28u32, 3, 0, 1, 1] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend([10, 20, 30, 40]);
    assert_eq!(
        dds::decode(Cursor::new(&bytes), &cancel())
            .unwrap()
            .pixels(),
        &[[10, 20, 30, 40]]
    );
    set(&mut bytes, 140, 2);
    assert!(dds::decode(Cursor::new(&bytes), &cancel()).is_err());
    set(&mut bytes, 140, 1);
    set(&mut bytes, 128, 77);
    set(&mut bytes, 144, 3);
    bytes.truncate(148);
    bytes.extend([0, 0, 0, 0, 0, 0, 0, 0, 0, 248, 0, 0, 0, 0, 0, 0]);
    assert_eq!(
        dds::decode(Cursor::new(bytes), &cancel()).unwrap().pixels(),
        &[[255, 0, 0, 255]]
    );
}

#[test]
fn bc4_and_bc5_unorm_preserve_channel_indices_and_dx10_layout() {
    let indices = (0..16).fold(0u64, |bits, i| bits | ((i % 8) << (i * 3)));
    let mut red = vec![255, 0];
    red.extend(&indices.to_le_bytes()[..6]);
    let mut green = vec![0, 255];
    green.extend(&indices.to_le_bytes()[..6]);
    let expected_red = [255, 0, 218, 182, 145, 109, 72, 36];
    let expected_green = [0, 255, 51, 102, 153, 204, 0, 255];
    for (fourcc, dxgi, two) in [
        (b"ATI1", 80, false),
        (b"BC4U", 80, false),
        (b"ATI2", 83, true),
        (b"BC5U", 83, true),
    ] {
        let mut blocks = red.clone();
        if two {
            blocks.extend(&green);
        }
        for dx10 in [false, true] {
            let bytes = if dx10 {
                let mut payload = Vec::new();
                for n in [dxgi, 3u32, 0, 1, 0] {
                    payload.extend(n.to_le_bytes());
                }
                payload.extend(&blocks);
                compressed(b"DX10", (4, 4), &payload)
            } else {
                compressed(fourcc, (4, 4), &blocks)
            };
            let doc = dds::decode(Cursor::new(&bytes), &cancel()).unwrap();
            for (i, p) in doc.pixels().iter().enumerate() {
                assert_eq!(
                    *p,
                    [
                        expected_red[i % 8],
                        if two { expected_green[i % 8] } else { 0 },
                        0,
                        255
                    ]
                );
            }
            for n in 1..=blocks.len() {
                assert!(dds::decode(Cursor::new(&bytes[..bytes.len() - n]), &cancel()).is_err());
            }
        }
    }
}

#[test]
fn bc5_partial_blocks_clip_edges_without_swapping_channels() {
    let block = [50, 50, 0, 0, 0, 0, 0, 0, 200, 200, 0, 0, 0, 0, 0, 0];
    let mut blocks = block.to_vec();
    blocks.extend([80, 80, 0, 0, 0, 0, 0, 0, 160, 160, 0, 0, 0, 0, 0, 0]);
    let doc = dds::decode(Cursor::new(compressed(b"ATI2", (5, 3), &blocks)), &cancel()).unwrap();
    for (i, p) in doc.pixels().iter().enumerate() {
        assert_eq!(
            *p,
            if i % 5 == 4 {
                [80, 160, 0, 255]
            } else {
                [50, 200, 0, 255]
            }
        );
    }
}

#[test]
fn mipmap_chain_has_complete_headers_dimensions_and_exact_base_pixels() {
    for size in [(7, 3), (1, 8), (8, 1), (1, 1)] {
        let pixels = vec![[10, 20, 30, 128]; (size.0 * size.1) as usize];
        let options = RasterOptions {
            dds_mipmaps: true,
            ..RasterOptions::new(RasterFormat::Dds)
        };
        let bytes = files::encode_raster_bytes(&pixels, size, options, &cancel()).unwrap();
        let word = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        let levels = size.0.max(size.1).ilog2() + 1;
        assert_eq!(word(28), levels);
        assert_eq!(word(8) & 0x20000 != 0, levels > 1);
        assert_eq!(word(108) & 0x400008 != 0, levels > 1);
        let (mut width, mut height, mut offset) = (size.0, size.1, 128);
        for _ in 0..levels {
            let end = offset + (width * height * 4) as usize;
            assert!(
                bytes[offset..end]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| *p == [10, 20, 30, 128])
            );
            offset = end;
            width = (width / 2).max(1);
            height = (height / 2).max(1);
        }
        assert_eq!(offset, bytes.len());
        assert_eq!(
            files::decode_raster_bytes(&bytes, RasterFormat::Dds, &cancel())
                .unwrap()
                .pixels(),
            pixels
        );
    }
}

#[test]
fn mipmaps_average_linear_light_and_do_not_leak_transparent_rgb() {
    let options = RasterOptions {
        dds_mipmaps: true,
        ..RasterOptions::new(RasterFormat::Dds)
    };
    for (pixels, expected) in [
        ([[0, 0, 0, 255], [255; 4]], [188, 188, 188, 255]),
        ([[255, 0, 0, 0], [0, 0, 255, 255]], [0, 0, 255, 128]),
    ] {
        let bytes = files::encode_raster_bytes(&pixels, (2, 1), options, &cancel()).unwrap();
        assert_eq!(&bytes[136..], expected);
        assert_eq!(&bytes[128..136], pixels.as_flattened());
    }
}

#[test]
fn mipmap_generation_checks_cancellation_between_levels() {
    struct CancelWriter<'a>(&'a AtomicBool, usize);
    impl std::io::Write for CancelWriter<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.1 += bytes.len();
            if self.1 >= 128 + 8 {
                self.0.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let flag = cancel();
    let mut writer = CancelWriter(&flag, 0);
    assert_eq!(
        dds::encode_with_mipmaps(&[[255; 4]; 2], (2, 1), &mut writer, true, &flag).unwrap_err(),
        files::CANCELLED
    );
    assert_eq!(writer.1, 136);
}

#[test]
fn externally_opened_mipmap_fixture_retains_its_base_surface() {
    let bytes = include_bytes!("fixtures/dds/rgba-mip-chain.dds");
    let document = dds::decode(Cursor::new(bytes), &cancel()).unwrap();
    assert_eq!((document.width, document.height), (32, 16));
    for (i, pixel) in document.pixels().iter().enumerate() {
        assert_eq!(
            *pixel,
            [
                (i % 32 * 8) as u8,
                (i / 32 * 16) as u8,
                80,
                if i % 32 < 8 { 64 } else { 255 }
            ]
        );
    }
    assert_eq!(u32::from_le_bytes(bytes[28..32].try_into().unwrap()), 6);
    assert_eq!(bytes.len(), 2860);
}

#[test]
fn compressed_export_handles_solid_colors_alpha_and_partial_edge_blocks() {
    for compression in [
        dds::Compression::Bc1,
        dds::Compression::Bc2,
        dds::Compression::Bc3,
    ] {
        for size in [(1, 1), (3, 7), (7, 5), (8, 8)] {
            for pixel in [
                [0, 0, 0, 255],
                [255; 4],
                [80, 120, 160, 128],
                [20, 40, 60, 0],
            ] {
                let pixels = vec![pixel; (size.0 * size.1) as usize];
                let bytes = files::encode_raster_bytes(
                    &pixels,
                    size,
                    RasterOptions {
                        dds_compression: compression,
                        ..RasterOptions::new(RasterFormat::Dds)
                    },
                    &cancel(),
                )
                .unwrap();
                let block_bytes = if compression == dds::Compression::Bc1 {
                    8
                } else {
                    16
                };
                assert_eq!(
                    bytes.len(),
                    128 + size.0.div_ceil(4) as usize * size.1.div_ceil(4) as usize * block_bytes
                );
                let document = dds::decode(Cursor::new(bytes), &cancel()).unwrap();
                assert_eq!((document.width, document.height), size);
                for actual in document.pixels() {
                    let alpha = if compression == dds::Compression::Bc1 {
                        if pixel[3] < 128 { 0 } else { 255 }
                    } else if compression == dds::Compression::Bc2 {
                        ((u16::from(pixel[3]) * 15 + 127) / 255 * 17) as u8
                    } else {
                        pixel[3]
                    };
                    assert_eq!(actual[3], alpha);
                    if alpha > 0 {
                        for c in 0..3 {
                            assert!(
                                actual[c].abs_diff(pixel[c]) <= 4,
                                "{compression:?} {pixel:?} {actual:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn compressed_mipmaps_have_block_aligned_payloads_and_preview_reopens() {
    for compression in [
        dds::Compression::Bc1,
        dds::Compression::Bc2,
        dds::Compression::Bc3,
    ] {
        let bytes = files::encode_raster_bytes(
            &[[180, 80, 40, 255]; 35],
            (7, 5),
            RasterOptions {
                dds_compression: compression,
                dds_mipmaps: true,
                ..RasterOptions::new(RasterFormat::Dds)
            },
            &cancel(),
        )
        .unwrap();
        let length = if compression == dds::Compression::Bc1 {
            8
        } else {
            16
        };
        assert_eq!(u32::from_le_bytes(bytes[28..32].try_into().unwrap()), 3);
        assert_eq!(bytes.len(), 128 + (4 + 1 + 1) * length);
        let document = files::decode_raster_bytes(&bytes, RasterFormat::Dds, &cancel()).unwrap();
        assert_eq!((document.width, document.height), (7, 5));
        assert!(!document.dirty());
        assert!(dds::decode(Cursor::new(&bytes[..128 + 4 * length - 1]), &cancel()).is_err());
    }
}

#[test]
fn compressed_exports_match_independent_ffmpeg_decode() {
    for (encoded, expected) in [
        (
            include_bytes!("fixtures/dds/export-bc1.dds").as_slice(),
            include_bytes!("fixtures/dds/export-bc1.rgba").as_slice(),
        ),
        (
            include_bytes!("fixtures/dds/export-bc2.dds").as_slice(),
            include_bytes!("fixtures/dds/export-bc2.rgba").as_slice(),
        ),
        (
            include_bytes!("fixtures/dds/export-bc3.dds").as_slice(),
            include_bytes!("fixtures/dds/export-bc3.rgba").as_slice(),
        ),
    ] {
        let actual = dds::decode(Cursor::new(encoded), &cancel()).unwrap();
        assert_eq!(actual.pixels().len() * 4, expected.len());
        for (actual, expected) in actual.pixels().iter().zip(expected.as_chunks::<4>().0) {
            assert_eq!(actual[3], expected[3]);
            for c in 0..3 {
                assert!(
                    actual[c].abs_diff(expected[c]) <= 1,
                    "{actual:?} != {expected:?}"
                );
            }
        }
    }
}

#[test]
fn bc3_export_matches_native_windows_visible_pixels_with_rounding_tolerance() {
    let actual = dds::decode(
        Cursor::new(include_bytes!("fixtures/dds/export-bc3.dds")),
        &cancel(),
    )
    .unwrap();
    let expected = include_bytes!("fixtures/dds/export-bc3-native.rgba");
    for (actual, expected) in actual.pixels().iter().zip(expected.as_chunks::<4>().0) {
        assert!(actual[3].abs_diff(expected[3]) <= 1);
        if actual[3] > 0 && expected[3] > 0 {
            for c in 0..3 {
                assert!(actual[c].abs_diff(expected[c]) <= 2);
            }
        }
    }
}

#[test]
fn data_compression_preserves_selected_channels_and_discards_alpha() {
    for compression in [dds::Compression::Bc4, dds::Compression::Bc5] {
        let bytes = files::encode_raster_bytes(
            &[[12, 243, 90, 0]; 35],
            (7, 5),
            RasterOptions {
                dds_compression: compression,
                ..RasterOptions::new(RasterFormat::Dds)
            },
            &cancel(),
        )
        .unwrap();
        let document = files::decode_raster_bytes(&bytes, RasterFormat::Dds, &cancel()).unwrap();
        assert_eq!((document.width, document.height), (7, 5));
        let expected = [
            12,
            if compression == dds::Compression::Bc5 {
                243
            } else {
                0
            },
            0,
            255,
        ];
        assert!(document.pixels().iter().all(|pixel| *pixel == expected));
    }
}
#[test]
fn data_mipmaps_average_values_without_alpha_weighting_or_srgb_conversion() {
    for compression in [dds::Compression::Bc4, dds::Compression::Bc5] {
        let bytes = files::encode_raster_bytes(
            &[[0, 255, 0, 0], [255, 0, 255, 255]],
            (2, 1),
            RasterOptions {
                dds_compression: compression,
                dds_mipmaps: true,
                ..RasterOptions::new(RasterFormat::Dds)
            },
            &cancel(),
        )
        .unwrap();
        let length = if compression == dds::Compression::Bc4 {
            8
        } else {
            16
        };
        assert_eq!(bytes.len(), 148 + 2 * length);
        let mut mip = bytes[..148].to_vec();
        set(&mut mip, 16, 1);
        set(&mut mip, 12, 1);
        mip.extend_from_slice(&bytes[148 + length..]);
        let document = dds::decode(Cursor::new(mip), &cancel()).unwrap();
        assert_eq!(
            document.pixels(),
            &[[
                128,
                if compression == dds::Compression::Bc5 {
                    128
                } else {
                    0
                },
                0,
                255
            ]]
        );
    }
}

#[test]
fn compressed_data_channels_match_independent_decoder() {
    for (encoded, expected, channels) in [
        (
            include_bytes!("fixtures/dds/export-bc4.dds").as_slice(),
            include_bytes!("fixtures/dds/export-bc4.rgba").as_slice(),
            1,
        ),
        (
            include_bytes!("fixtures/dds/export-bc5.dds").as_slice(),
            include_bytes!("fixtures/dds/export-bc5.rgba").as_slice(),
            2,
        ),
    ] {
        let actual = dds::decode(Cursor::new(encoded), &cancel()).unwrap();
        assert_eq!(actual.pixels().len() * 4, expected.len());
        for (actual, expected) in actual.pixels().iter().zip(expected.as_chunks::<4>().0) {
            assert_eq!(actual[3], 255);
            for c in 0..channels {
                assert!(
                    actual[c].abs_diff(expected[c]) <= 1,
                    "{actual:?} != {expected:?}"
                );
            }
        }
    }
}

#[test]
fn bc2_alpha_ramp_uses_all_sixteen_levels_and_bounded_error() {
    let pixels: Vec<_> = (0..=255).map(|alpha| [255, 0, 0, alpha]).collect();
    let bytes = files::encode_raster_bytes(
        &pixels,
        (16, 16),
        RasterOptions {
            dds_compression: dds::Compression::Bc2,
            ..RasterOptions::new(RasterFormat::Dds)
        },
        &cancel(),
    )
    .unwrap();
    assert_eq!(&bytes[84..88], b"DXT3");
    let decoded = dds::decode(Cursor::new(bytes), &cancel()).unwrap();
    let mut seen = [false; 16];
    for (original, actual) in pixels.iter().zip(decoded.pixels()) {
        assert!(original[3].abs_diff(actual[3]) <= 8);
        assert_eq!(actual[3] % 17, 0);
        seen[usize::from(actual[3] / 17)] = true;
        if actual[3] > 0 {
            assert_eq!(&actual[..3], &[255, 0, 0]);
        }
    }
    assert!(seen.into_iter().all(|value| value));
}
