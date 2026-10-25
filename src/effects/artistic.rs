use super::{AtomicBool, Effect, Ordering, Pixel};
fn luminance(pixel: Pixel) -> f64 {
    f64::from(pixel[0]) * 0.2126 + f64::from(pixel[1]) * 0.7152 + f64::from(pixel[2]) * 0.0722
}
pub(super) fn render(
    source: &[Pixel],
    width: u32,
    height: u32,
    effect: Effect,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    if !effect.valid_settings() {
        return None;
    }
    if let Effect::OilPainting { radius, bins } = effect {
        return oil(source, width, height, radius, bins, cancel);
    }
    if matches!(effect, Effect::PencilSketch { strength: 0.0, .. }) {
        return Some(source.to_vec());
    }
    let filtered = match effect {
        Effect::PencilSketch { radius, .. } => {
            super::blurs::gaussian(source, width, height, radius, cancel)?
        }
        Effect::InkSketch { .. } => super::stylize::edges(source, width, height, None, cancel)?,
        _ => return None,
    };
    let mut output = source.to_vec();
    for (i, (pixel, filter)) in source.iter().zip(filtered).enumerate() {
        if i.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
            return None;
        }
        if pixel[3] == 0 {
            continue;
        }
        let gray = luminance(*pixel);
        match effect {
            Effect::PencilSketch { strength, .. } => {
                let blurred = luminance(filter);
                let sketch = if blurred <= 0.0 {
                    255.0
                } else {
                    (gray * 255.0 / blurred).min(255.0)
                };
                for c in 0..3 {
                    output[i][c] = (f64::from(pixel[c])
                        + (sketch - f64::from(pixel[c])) * f64::from(strength))
                    .round()
                    .clamp(0.0, 255.0) as u8;
                }
            }
            Effect::InkSketch { threshold, color } => {
                let edge = filter[..3].iter().copied().max().unwrap();
                for c in 0..3 {
                    output[i][c] = if edge > threshold {
                        0
                    } else {
                        (gray + (f64::from(pixel[c]) - gray) * f64::from(color))
                            .round()
                            .clamp(0.0, 255.0) as u8
                    };
                }
            }
            _ => unreachable!(),
        }
    }
    Some(output)
}
fn oil(
    source: &[Pixel],
    width: u32,
    height: u32,
    radius: u32,
    levels: u8,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    if radius == 0 {
        return Some(source.to_vec());
    }
    let (w, h, r) = (i64::from(width), i64::from(height), i64::from(radius));
    let mut output = source.to_vec();
    for y in 0..h {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let mut bins = [[0u64; 4]; 64];
        let column = |x: i64, add: bool, bins: &mut [[u64; 4]; 64]| {
            for sy in y - r..=y + r {
                let pixel = source[(sy.clamp(0, h - 1) * w + x.clamp(0, w - 1)) as usize];
                let luma = (54 * u32::from(pixel[0])
                    + 183 * u32::from(pixel[1])
                    + 19 * u32::from(pixel[2]))
                    >> 8;
                let bin = (luma * u32::from(levels) / 256) as usize;
                let alpha = u64::from(pixel[3]);
                for c in 0..4 {
                    let value = if c == 3 {
                        alpha
                    } else {
                        u64::from(pixel[c]) * alpha
                    };
                    if add {
                        bins[bin][c] += value;
                    } else {
                        bins[bin][c] -= value;
                    }
                }
            }
        };
        for x in -r..=r {
            column(x, true, &mut bins);
        }
        for x in 0..w {
            if x % 256 == 0 && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let index = (y * w + x) as usize;
            if source[index][3] > 0 {
                let best = bins[..levels as usize]
                    .iter()
                    .enumerate()
                    .max_by_key(|(i, bin)| (bin[3], std::cmp::Reverse(*i)))
                    .unwrap()
                    .1;
                for c in 0..3 {
                    output[index][c] =
                        (best[c] + best[3] / 2).checked_div(best[3]).unwrap_or(0) as u8;
                }
            }
            column(x - r, false, &mut bins);
            column(x + r + 1, true, &mut bins);
        }
    }
    Some(output)
}
