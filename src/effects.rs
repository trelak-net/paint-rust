use crate::document::{Pixel, Selection};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    BrightnessContrast { brightness: i16, contrast: i16 },
    Saturation(f32),
    Grayscale,
    Sepia,
    BoxBlur(u32),
}
impl Effect {
    pub fn name(self) -> &'static str {
        match self {
            Self::BrightnessContrast { .. } => "Brightness / Contrast",
            Self::Saturation(_) => "Saturation",
            Self::Grayscale => "Grayscale",
            Self::Sepia => "Sepia",
            Self::BoxBlur(_) => "Box blur",
        }
    }
}

pub fn render(
    source: &[Pixel],
    width: u32,
    height: u32,
    selection: Option<Selection>,
    effect: Effect,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    let mut output = match effect {
        Effect::BoxBlur(radius) if radius > 0 => {
            blur(source, width, height, radius.min(64), cancel)?
        }
        _ => source.to_vec(),
    };
    for y in 0..height as usize {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        for x in 0..width as usize {
            let i = y * width as usize + x;
            if selection.is_some_and(|s| !s.contains(x, y)) {
                output[i] = source[i];
                continue;
            }
            let p = source[i];
            let luminance =
                0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2]);
            for (c, channel) in p[..3].iter().enumerate() {
                let value = match effect {
                    Effect::BrightnessContrast {
                        brightness,
                        contrast,
                    } => {
                        let c = f64::from(contrast.clamp(-100, 100)) * 2.55;
                        let factor = 259.0 * (c + 255.0) / (255.0 * (259.0 - c));
                        (f64::from(*channel) - 127.5) * factor
                            + 127.5
                            + f64::from(brightness.clamp(-100, 100)) * 2.55
                    }
                    Effect::Saturation(amount) => {
                        luminance
                            + (f64::from(*channel) - luminance) * f64::from(amount.clamp(0.0, 2.0))
                    }
                    Effect::Grayscale => luminance,
                    Effect::Sepia => luminance * [1.13, 1.0, 0.8][c],
                    Effect::BoxBlur(_) => continue,
                };
                output[i][c] = value.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Some(output)
}

fn premultiply(pixel: Pixel) -> [f64; 4] {
    let alpha = f64::from(pixel[3]);
    [
        f64::from(pixel[0]) * alpha,
        f64::from(pixel[1]) * alpha,
        f64::from(pixel[2]) * alpha,
        alpha,
    ]
}

fn blur(
    source: &[Pixel],
    width: u32,
    height: u32,
    radius: u32,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    let (width, height, radius) = (width as usize, height as usize, radius as i64);
    let count = (radius * 2 + 1) as f64;
    let mut horizontal = vec![[0.0_f32; 4]; source.len()];
    for y in 0..height {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let sample =
            |x: i64| premultiply(source[y * width + x.clamp(0, width as i64 - 1) as usize]);
        let mut sum = [0.0; 4];
        for x in -radius..=radius {
            let p = sample(x);
            for c in 0..4 {
                sum[c] += p[c];
            }
        }
        for x in 0..width {
            for (c, value) in sum.iter().enumerate() {
                horizontal[y * width + x][c] = (value / count) as f32;
            }
            let removed = sample(x as i64 - radius);
            let added = sample(x as i64 + radius + 1);
            for c in 0..4 {
                sum[c] += added[c] - removed[c];
            }
        }
    }
    let mut output = vec![[0; 4]; source.len()];
    for x in 0..width {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let sample = |y: i64| horizontal[y.clamp(0, height as i64 - 1) as usize * width + x];
        let mut sum = [0.0; 4];
        for y in -radius..=radius {
            let p = sample(y);
            for c in 0..4 {
                sum[c] += f64::from(p[c]);
            }
        }
        for y in 0..height {
            let dst = &mut output[y * width + x];
            if sum[3] > 0.0 {
                for c in 0..3 {
                    dst[c] = (sum[c] / sum[3]).round().clamp(0.0, 255.0) as u8;
                }
                dst[3] = (sum[3] / count).round().clamp(0.0, 255.0) as u8;
            }
            let removed = sample(y as i64 - radius);
            let added = sample(y as i64 + radius + 1);
            for c in 0..4 {
                sum[c] += f64::from(added[c]) - f64::from(removed[c]);
            }
        }
    }
    Some(output)
}
