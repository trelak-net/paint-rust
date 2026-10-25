//! Bounded top-surface DDS import and RGBA8 export with optional mipmaps.
use crate::{document::Document, files::CANCELLED};
use std::io::{Read, Seek, Write};
use std::sync::atomic::{AtomicBool, Ordering};

fn check(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}
fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

pub fn decode(mut reader: impl Read + Seek, cancel: &AtomicBool) -> Result<Document, String> {
    check(cancel)?;
    let mut header = [0; 128];
    reader.read_exact(&mut header).map_err(|e| e.to_string())?;
    if &header[..4] != b"DDS " || word(&header, 4) != 124 || word(&header, 76) != 32 {
        return Err("Invalid DDS header".into());
    }
    let (width, height) = (word(&header, 16), word(&header, 12));
    Document::validate_size(width, height)?;
    if word(&header, 112) & 0x20fe00 != 0 || word(&header, 24) > 1 {
        return Err("DDS cube maps and volume textures are not supported".into());
    }
    let flags = word(&header, 80);
    let mut bits = word(&header, 88);
    let mut masks = [
        word(&header, 92),
        word(&header, 96),
        word(&header, 100),
        word(&header, 104),
    ];
    if flags & 1 == 0 {
        masks[3] = 0;
    }
    let mut compression = 0;
    let mut opaque_alpha = false;
    let compressed = if flags & 4 != 0 {
        if &header[84..88] == b"DX10" {
            let mut extension = [0; 20];
            reader
                .read_exact(&mut extension)
                .map_err(|e| e.to_string())?;
            if word(&extension, 4) != 3 || word(&extension, 8) & 4 != 0 || word(&extension, 12) != 1
            {
                return Err("Only single 2D DDS textures are supported".into());
            }
            let format = word(&extension, 0);
            opaque_alpha = word(&extension, 16) & 7 == 3;
            if word(&extension, 16) & 7 == 2 {
                return Err("Premultiplied DDS alpha is not supported".into());
            }
            compression = match format {
                70..=72 => 1,
                73..=75 => 2,
                76..=78 => 3,
                79 | 80 => 4,
                82 | 83 => 5,
                _ => 0,
            };
            if matches!(format, 28 | 29 | 87 | 88 | 91 | 93) {
                bits = 32;
                masks = if matches!(format, 28 | 29) {
                    [0xff, 0xff00, 0xff0000, 0xff000000]
                } else {
                    [0xff0000, 0xff00, 0xff, 0xff000000]
                };
                if matches!(format, 88 | 93) || word(&extension, 16) & 7 == 3 {
                    masks[3] = 0;
                }
                false
            } else {
                true
            }
        } else {
            compression = match &header[84..88] {
                b"DXT1" => 1,
                b"DXT3" => 2,
                b"DXT5" => 3,
                b"ATI1" | b"BC4U" => 4,
                b"ATI2" | b"BC5U" => 5,
                _ => 0,
            };
            true
        }
    } else {
        if flags & 0x40 == 0 {
            return Err("Unsupported DDS pixel format".into());
        }
        false
    };
    if compressed {
        if compression == 0 {
            return Err("Unsupported DDS block compression".into());
        }
        return decode_blocks(reader, width, height, compression, opaque_alpha, cancel);
    }
    if !matches!(bits, 16 | 24 | 32) {
        return Err("Unsupported DDS RGB bit depth".into());
    }
    let mut used = 0u32;
    for (index, mask) in masks.into_iter().enumerate() {
        if mask == 0 {
            if index < 3 {
                return Err("Missing DDS color channel".into());
            }
            continue;
        }
        let normalized = mask >> mask.trailing_zeros();
        if normalized & normalized.wrapping_add(1) != 0
            || mask & used != 0
            || (bits < 32 && mask >> bits != 0)
        {
            return Err("Invalid DDS channel masks".into());
        }
        used |= mask;
    }
    let pixel_bytes = bits as usize / 8;
    let row_bytes = width as usize * pixel_bytes;
    let pitch = if word(&header, 8) & 8 != 0 {
        word(&header, 20) as usize
    } else {
        row_bytes
    };
    if pitch < row_bytes || pitch > row_bytes + 4096 {
        return Err("Invalid or excessive DDS row pitch".into());
    }
    let mut row = vec![0u8; pitch];
    let mut pixels = Vec::with_capacity(width as usize * height as usize);
    for _ in 0..height {
        check(cancel)?;
        reader.read_exact(&mut row).map_err(|e| e.to_string())?;
        for bytes in row[..row_bytes].chunks_exact(pixel_bytes) {
            let mut value = [0u8; 4];
            value[..pixel_bytes].copy_from_slice(bytes);
            let value = u32::from_le_bytes(value);
            let mut pixel = [255; 4];
            for (channel, mask) in masks.into_iter().enumerate() {
                if mask != 0 {
                    let shift = mask.trailing_zeros();
                    let maximum = u64::from(mask >> shift);
                    pixel[channel] =
                        ((u64::from((value & mask) >> shift) * 255 + maximum / 2) / maximum) as u8;
                }
            }
            pixels.push(pixel);
        }
    }
    check(cancel)?;
    Document::from_pixels(width, height, pixels)
}

pub fn encode(
    pixels: &[[u8; 4]],
    size: (u32, u32),
    writer: impl Write,
    cancel: &AtomicBool,
) -> Result<(), String> {
    encode_with_mipmaps(pixels, size, writer, false, cancel)
}

pub fn encode_with_mipmaps(
    pixels: &[[u8; 4]],
    size: (u32, u32),
    mut writer: impl Write,
    mipmaps: bool,
    cancel: &AtomicBool,
) -> Result<(), String> {
    check(cancel)?;
    Document::validate_size(size.0, size.1)?;
    if pixels.len() != size.0 as usize * size.1 as usize {
        return Err("Invalid DDS pixel count".into());
    }
    let levels = if mipmaps {
        size.0.max(size.1).ilog2() + 1
    } else {
        1
    };
    let mut header = [0u8; 128];
    header[..4].copy_from_slice(b"DDS ");
    for (offset, value) in [
        (4, 124u32),
        (8, 0x100f | if levels > 1 { 0x20000 } else { 0 }),
        (12, size.1),
        (16, size.0),
        (20, size.0 * 4),
        (28, if mipmaps { levels } else { 0 }),
        (76, 32),
        (80, 0x41),
        (88, 32),
        (92, 0xff),
        (96, 0xff00),
        (100, 0xff0000),
        (104, 0xff000000),
        (108, 0x1000 | if levels > 1 { 0x400008 } else { 0 }),
    ] {
        header[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    writer.write_all(&header).map_err(|e| e.to_string())?;
    let mut dimensions = size;
    let mut level_pixels = std::borrow::Cow::Borrowed(pixels);
    for level in 0..levels {
        for row in level_pixels.chunks(dimensions.0 as usize) {
            check(cancel)?;
            let bytes: Vec<_> = row.iter().flatten().copied().collect();
            writer.write_all(&bytes).map_err(|e| e.to_string())?;
        }
        if level + 1 < levels {
            let next_size = ((dimensions.0 / 2).max(1), (dimensions.1 / 2).max(1));
            let next = crate::resample::resize(
                &level_pixels,
                dimensions,
                next_size,
                crate::resample::Options {
                    filter: crate::resample::Filter::Area,
                    gamma_correct: true,
                },
                cancel,
            )?;
            level_pixels = std::borrow::Cow::Owned(next);
            dimensions = next_size;
        }
    }
    check(cancel)
}

fn decode_blocks(
    mut reader: impl Read,
    width: u32,
    height: u32,
    format: u8,
    opaque_alpha: bool,
    cancel: &AtomicBool,
) -> Result<Document, String> {
    let mut pixels = vec![[0; 4]; width as usize * height as usize];
    let length = if matches!(format, 1 | 4) { 8 } else { 16 };
    for by in 0..height.div_ceil(4) {
        check(cancel)?;
        for bx in 0..width.div_ceil(4) {
            let mut block = [0u8; 16];
            reader
                .read_exact(&mut block[..length])
                .map_err(|e| e.to_string())?;
            if matches!(format, 4 | 5) {
                let red = decode_channel(&block[..8]);
                let green = if format == 5 {
                    decode_channel(&block[8..])
                } else {
                    [0; 16]
                };
                for i in 0..16u32 {
                    let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                    if x < width && y < height {
                        pixels[y as usize * width as usize + x as usize] =
                            [red[i as usize], green[i as usize], 0, 255];
                    }
                }
                continue;
            }
            let colors = &block[length - 8..length];
            let c0 = u16::from_le_bytes([colors[0], colors[1]]);
            let c1 = u16::from_le_bytes([colors[2], colors[3]]);
            let expand = |c: u16| {
                [
                    (((c >> 11) * 255 + 15) / 31) as u8,
                    ((((c >> 5) & 63) * 255 + 31) / 63) as u8,
                    (((c & 31) * 255 + 15) / 31) as u8,
                    255,
                ]
            };
            let mut palette = [expand(c0), expand(c1), [0, 0, 0, 255], [0; 4]];
            for channel in 0..3 {
                let (a, b) = (
                    u16::from(palette[0][channel]),
                    u16::from(palette[1][channel]),
                );
                if format != 1 || c0 > c1 {
                    palette[2][channel] = ((2 * a + b) / 3) as u8;
                    palette[3][channel] = ((a + 2 * b) / 3) as u8;
                    palette[3][3] = 255;
                } else {
                    palette[2][channel] = ((a + b) / 2) as u8;
                }
            }
            let indices = word(colors, 4);
            let mut alpha = [0u8; 8];
            alpha[0] = block[0];
            alpha[1] = block[1];
            let divisor = if alpha[0] > alpha[1] { 7 } else { 5 };
            for i in 1..divisor {
                alpha[i + 1] = (((divisor - i) as u16 * u16::from(alpha[0])
                    + i as u16 * u16::from(alpha[1]))
                    / divisor as u16) as u8;
            }
            if divisor == 5 {
                alpha[6] = 0;
                alpha[7] = 255;
            }
            let alpha_bits = u64::from_le_bytes(block[..8].try_into().unwrap());
            for i in 0..16 {
                let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                if x >= width || y >= height {
                    continue;
                }
                let mut pixel = palette[((indices >> (i * 2)) & 3) as usize];
                if format == 2 {
                    pixel[3] = (((alpha_bits >> (i * 4)) & 15) * 17) as u8;
                }
                if format == 3 {
                    pixel[3] = alpha[((alpha_bits >> (16 + i * 3)) & 7) as usize];
                }
                if opaque_alpha {
                    pixel[3] = 255;
                }
                pixels[y as usize * width as usize + x as usize] = pixel;
            }
        }
    }
    check(cancel)?;
    Document::from_pixels(width, height, pixels)
}

fn decode_channel(block: &[u8]) -> [u8; 16] {
    let mut palette = [0u8; 8];
    palette[0] = block[0];
    palette[1] = block[1];
    let divisor = if palette[0] > palette[1] { 7 } else { 5 };
    for i in 1..divisor {
        palette[i + 1] = (((divisor - i) as u16 * u16::from(palette[0])
            + i as u16 * u16::from(palette[1]))
            / divisor as u16) as u8;
    }
    if divisor == 5 {
        palette[7] = 255;
    }
    let bits = u64::from_le_bytes(block.try_into().unwrap()) >> 16;
    std::array::from_fn(|i| palette[((bits >> (i * 3)) & 7) as usize])
}
