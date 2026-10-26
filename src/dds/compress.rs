use super::*;
fn rgb565(pixel: [u8; 4]) -> u16 {
    ((u16::from(pixel[0]) * 31 + 127) / 255) << 11
        | ((u16::from(pixel[1]) * 63 + 127) / 255) << 5
        | ((u16::from(pixel[2]) * 31 + 127) / 255)
}
fn expand(value: u16) -> [u8; 3] {
    [
        (((value >> 11) * 255 + 15) / 31) as u8,
        ((((value >> 5) & 63) * 255 + 31) / 63) as u8,
        (((value & 31) * 255 + 15) / 31) as u8,
    ]
}
fn distance(a: &[u8], b: &[u8]) -> u32 {
    (0..3)
        .map(|c| {
            let d = i32::from(a[c]) - i32::from(b[c]);
            (d * d) as u32
        })
        .sum()
}
fn colors(pixels: &[[u8; 4]; 16], alpha: bool) -> [u8; 8] {
    let transparent = alpha && pixels.iter().any(|p| p[3] < 128);
    let visible = |p: &[u8; 4]| if transparent { p[3] >= 128 } else { p[3] > 0 };
    let mut endpoints = pixels
        .iter()
        .find(|p| visible(p))
        .map(|p| [rgb565(*p); 2])
        .unwrap_or([0; 2]);
    let mut maximum = 0;
    for (i, a) in pixels.iter().enumerate().filter(|(_, p)| visible(p)) {
        for b in pixels[i + 1..].iter().filter(|p| visible(p)) {
            let error = distance(&a[..3], &b[..3]);
            if error > maximum {
                maximum = error;
                endpoints = [rgb565(*a), rgb565(*b)];
            }
        }
    }
    endpoints.sort_unstable();
    if !transparent {
        endpoints.swap(0, 1);
        if endpoints[0] == endpoints[1] {
            if endpoints[0] < u16::MAX {
                endpoints[0] += 1;
            } else {
                endpoints[1] -= 1;
            }
        }
    }
    let mut palette = [expand(endpoints[0]), expand(endpoints[1]), [0; 3], [0; 3]];
    let [first, second, _, _] = palette;
    for (c, (a, b)) in first.into_iter().zip(second).enumerate() {
        let (a, b) = (u16::from(a), u16::from(b));
        if transparent {
            palette[2][c] = ((a + b) / 2) as u8;
        } else {
            palette[2][c] = ((2 * a + b) / 3) as u8;
            palette[3][c] = ((a + 2 * b) / 3) as u8;
        }
    }
    let mut indices = 0u32;
    for (i, pixel) in pixels.iter().enumerate() {
        let index = if transparent && pixel[3] < 128 {
            3
        } else {
            (0..if transparent { 3 } else { 4 })
                .min_by_key(|&index| distance(&pixel[..3], &palette[index]))
                .unwrap()
        };
        indices |= (index as u32) << (2 * i);
    }
    let mut output = [0; 8];
    output[..2].copy_from_slice(&endpoints[0].to_le_bytes());
    output[2..4].copy_from_slice(&endpoints[1].to_le_bytes());
    output[4..].copy_from_slice(&indices.to_le_bytes());
    output
}
fn channel(pixels: &[[u8; 4]; 16], component: usize) -> [u8; 8] {
    let high = pixels.iter().map(|p| p[component]).max().unwrap();
    let low = pixels.iter().map(|p| p[component]).min().unwrap();
    let mut palette = [high, low, 0, 0, 0, 0, 0, 0];
    if high > low {
        for i in 1..7 {
            palette[i + 1] =
                (((7 - i) as u16 * u16::from(high) + i as u16 * u16::from(low)) / 7) as u8;
        }
    } else {
        palette.fill(high);
    }
    let mut bits = u64::from(high) | (u64::from(low) << 8);
    for (i, pixel) in pixels.iter().enumerate() {
        let index = (0..8)
            .min_by_key(|&index| pixel[component].abs_diff(palette[index]))
            .unwrap();
        bits |= (index as u64) << (16 + 3 * i);
    }
    bits.to_le_bytes()
}
pub(super) fn write(
    pixels: &[[u8; 4]],
    size: (u32, u32),
    writer: &mut impl Write,
    compression: Compression,
    cancel: &AtomicBool,
) -> Result<(), String> {
    for by in 0..size.1.div_ceil(4) {
        for bx in 0..size.0.div_ceil(4) {
            check(cancel)?;
            let block = std::array::from_fn(|i| {
                let x = (bx * 4 + i as u32 % 4).min(size.0 - 1);
                let y = (by * 4 + i as u32 / 4).min(size.1 - 1);
                pixels[y as usize * size.0 as usize + x as usize]
            });
            if compression.data_channels() {
                writer
                    .write_all(&channel(&block, 0))
                    .map_err(|e| e.to_string())?;
                if compression == Compression::Bc5 {
                    writer
                        .write_all(&channel(&block, 1))
                        .map_err(|e| e.to_string())?;
                }
                continue;
            }
            if compression == Compression::Bc2 {
                let alpha = block.iter().enumerate().fold(0_u64, |bits, (i, pixel)| {
                    bits | (((u64::from(pixel[3]) * 15 + 127) / 255) << (4 * i))
                });
                writer
                    .write_all(&alpha.to_le_bytes())
                    .map_err(|e| e.to_string())?;
            }
            if compression == Compression::Bc3 {
                writer
                    .write_all(&channel(&block, 3))
                    .map_err(|e| e.to_string())?;
            }
            writer
                .write_all(&colors(&block, compression == Compression::Bc1))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
