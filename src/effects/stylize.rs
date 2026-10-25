use super::{AtomicBool, Ordering, Pixel};

fn hash(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846ca68b);
    value ^ (value >> 16)
}

pub(super) fn noise(
    pixel: Pixel,
    index: usize,
    amount: u8,
    coverage: u8,
    monochrome: bool,
    seed: u32,
) -> Pixel {
    if pixel[3] == 0
        || amount == 0
        || hash((index as u32) ^ seed) % 100 >= u32::from(coverage.min(100))
    {
        return pixel;
    }
    let mut output = pixel;
    for c in 0..3 {
        let channel = if monochrome { 0 } else { c as u32 };
        let random = hash(
            (index as u32)
                .wrapping_mul(17)
                .wrapping_add(channel)
                .wrapping_add(seed.wrapping_mul(31)),
        );
        let offset = (f64::from(random) / f64::from(u32::MAX) * 2.0 - 1.0)
            * f64::from(amount.min(100))
            * 2.55;
        output[c] = (f64::from(pixel[c]) + offset).round().clamp(0.0, 255.0) as u8;
    }
    output
}

pub(super) fn median(
    source: &[Pixel],
    width: u32,
    height: u32,
    radius: u32,
    percentile: u8,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    let radius = radius.min(10) as i64;
    if radius == 0 {
        return Some(source.to_vec());
    }
    let (w, h) = (i64::from(width), i64::from(height));
    let mut output = source.to_vec();
    for y in 0..h {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let mut bins = [[0_u32; 256]; 3];
        let mut total = 0_u32;
        let column = |x: i64, add: bool, bins: &mut [[u32; 256]; 3], total: &mut u32| {
            for sy in y - radius..=y + radius {
                let pixel = source[(sy.clamp(0, h - 1) * w + x.clamp(0, w - 1)) as usize];
                let weight = u32::from(pixel[3]);
                if add {
                    *total += weight;
                } else {
                    *total -= weight;
                }
                for c in 0..3 {
                    if add {
                        bins[c][pixel[c] as usize] += weight;
                    } else {
                        bins[c][pixel[c] as usize] -= weight;
                    }
                }
            }
        };
        for x in -radius..=radius {
            column(x, true, &mut bins, &mut total);
        }
        for x in 0..w {
            if x % 256 == 0 && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let index = (y * w + x) as usize;
            if total > 0 && source[index][3] > 0 {
                let target = ((u64::from(total - 1) * u64::from(percentile.min(100))) / 100) as u32;
                for (c, channel) in bins.iter().enumerate() {
                    let mut accumulated = 0;
                    output[index][c] = channel
                        .iter()
                        .position(|count| {
                            accumulated += count;
                            accumulated > target
                        })
                        .unwrap() as u8;
                }
            }
            column(x - radius, false, &mut bins, &mut total);
            column(x + radius + 1, true, &mut bins, &mut total);
        }
    }
    Some(output)
}

pub(super) fn edges(
    source: &[Pixel],
    width: u32,
    height: u32,
    angle: Option<f32>,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    if angle.is_some_and(|a| !a.is_finite()) {
        return None;
    }
    let radians = f64::from(angle.unwrap_or(0.0)).to_radians();
    let mut output = source.to_vec();
    let (w, h) = (i64::from(width), i64::from(height));
    for y in 0..h {
        for x in 0..w {
            if x % 256 == 0 && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let index = (y * w + x) as usize;
            if source[index][3] == 0 {
                continue;
            }
            let mut gx = [0.0; 3];
            let mut gy = [0.0; 3];
            for dy in -1_i64..=1 {
                for dx in -1_i64..=1 {
                    let pixel =
                        source[((y + dy).clamp(0, h - 1) * w + (x + dx).clamp(0, w - 1)) as usize];
                    let wx = dx as f64 * if dy == 0 { 2.0 } else { 1.0 };
                    let wy = dy as f64 * if dx == 0 { 2.0 } else { 1.0 };
                    for c in 0..3 {
                        let value = f64::from(pixel[c]) * f64::from(pixel[3]) / 255.0;
                        gx[c] += value * wx;
                        gy[c] += value * wy;
                    }
                }
            }
            if angle.is_some() {
                let luminance =
                    |values: [f64; 3]| values[0] * 0.2126 + values[1] * 0.7152 + values[2] * 0.0722;
                let value = (128.0
                    + (luminance(gx) * radians.cos() + luminance(gy) * radians.sin()) / 4.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                output[index] = [value, value, value, source[index][3]];
            } else {
                for c in 0..3 {
                    output[index][c] = (gx[c].hypot(gy[c]) / 4.0).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
    Some(output)
}
