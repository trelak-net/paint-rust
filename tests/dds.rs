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
