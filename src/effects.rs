use crate::document::{Pixel, Selection};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    Levels(crate::levels::Levels),
    AutoLevels(f32),
    Move(crate::movement::Parameters),
    Shape(crate::shapes::Parameters),
    Gradient(crate::gradient::Gradient),
    FreeTransform(crate::affine::FreeTransform),
    BrightnessContrast {
        brightness: i16,
        contrast: i16,
    },
    Saturation(f32),
    HueSaturation {
        hue: i16,
        saturation: f32,
        lightness: i16,
    },
    Grayscale,
    Sepia,
    BoxBlur(u32),
    Pixelate(u32),
    Posterize(u8),
    Sharpen(f32),
}
impl Effect {
    pub fn repeatable(self) -> bool {
        !matches!(
            self,
            Self::Move(_) | Self::Shape(_) | Self::Gradient(_) | Self::FreeTransform(_)
        )
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Levels(_) => "Levels",
            Self::AutoLevels(_) => "Auto levels",
            Self::Move(parameters) => {
                if parameters.copy {
                    "Copy pixels"
                } else {
                    "Move pixels"
                }
            }
            Self::Shape(parameters) => match parameters.shape {
                crate::document::Shape::Line => "Line",
                crate::document::Shape::Rectangle => "Rectangle",
                crate::document::Shape::Ellipse => "Ellipse",
            },
            Self::Gradient(_) => "Gradient",
            Self::FreeTransform(_) => "Free transform",
            Self::BrightnessContrast { .. } => "Brightness / Contrast",
            Self::Saturation(_) => "Saturation",
            Self::HueSaturation { .. } => "Hue / Saturation / Lightness",
            Self::Grayscale => "Grayscale",
            Self::Sepia => "Sepia",
            Self::BoxBlur(_) => "Box blur",
            Self::Pixelate(_) => "Pixelate",
            Self::Posterize(_) => "Posterize",
            Self::Sharpen(_) => "Sharpen",
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
    if cancel.load(Ordering::Relaxed)
        || width == 0
        || height == 0
        || u64::from(width) * u64::from(height) != source.len() as u64
    {
        return None;
    }
    if let Effect::Move(parameters) = effect {
        return crate::movement::render(
            source,
            width,
            height,
            selection.as_ref(),
            parameters,
            cancel,
        );
    }
    if let Effect::Shape(parameters) = effect {
        return crate::shapes::render(
            source,
            width,
            height,
            selection.as_ref(),
            parameters,
            cancel,
        );
    }
    if let Effect::Gradient(parameters) = effect {
        return crate::gradient::render(
            source,
            width,
            height,
            selection.as_ref(),
            parameters,
            cancel,
        );
    }
    if let Effect::FreeTransform(parameters) = effect {
        return crate::affine::render(
            source,
            width,
            height,
            selection.as_ref(),
            parameters,
            cancel,
        );
    }
    if let Effect::HueSaturation { saturation, .. } = effect
        && !saturation.is_finite()
    {
        return None;
    }
    let level_tables = match effect {
        Effect::Levels(parameters) => Some(parameters.tables()?),
        Effect::AutoLevels(clip) => Some(
            crate::levels::automatic(source, width, selection.as_ref(), clip, cancel)?.tables()?,
        ),
        _ => None,
    };
    let mut output = match effect {
        Effect::BoxBlur(radius) if radius > 0 => {
            blur(source, width, height, radius.min(64), cancel)?
        }
        Effect::Sharpen(amount) if amount > 0.0 => blur(source, width, height, 1, cancel)?,
        Effect::Pixelate(size) if size > 1 => {
            pixelate(source, width, height, size.min(128), cancel)?
        }
        _ => source.to_vec(),
    };
    for y in 0..height as usize {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        for x in 0..width as usize {
            if x.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let i = y * width as usize + x;
            if selection.as_ref().is_some_and(|s| !s.contains(x, y)) {
                output[i] = source[i];
                continue;
            }
            let p = source[i];
            if let Some(tables) = &level_tables {
                for channel in 0..3 {
                    output[i][channel] = tables[channel][p[channel] as usize];
                }
                continue;
            }
            if let Effect::HueSaturation {
                hue,
                saturation,
                lightness,
            } = effect
            {
                output[i] = adjust_hsl(p, hue, saturation, lightness);
                continue;
            }
            if matches!(effect, Effect::Sharpen(_)) {
                output[i][3] = p[3];
            }
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
                    Effect::Sharpen(amount) => {
                        if p[3] == 0 {
                            f64::from(*channel)
                        } else {
                            f64::from(*channel)
                                + f64::from(amount.clamp(0.0, 3.0))
                                    * (f64::from(*channel) - f64::from(output[i][c]))
                        }
                    }
                    Effect::Posterize(levels) => {
                        let steps = f64::from(levels.clamp(2, 32) - 1);
                        (f64::from(*channel) * steps / 255.0).round() * 255.0 / steps
                    }
                    Effect::Levels(_)
                    | Effect::AutoLevels(_)
                    | Effect::HueSaturation { .. }
                    | Effect::BoxBlur(_)
                    | Effect::Pixelate(_)
                    | Effect::FreeTransform(_)
                    | Effect::Gradient(_)
                    | Effect::Shape(_)
                    | Effect::Move(_) => continue,
                };
                output[i][c] = value.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Some(output)
}

fn adjust_hsl(pixel: Pixel, hue: i16, saturation: f32, lightness: i16) -> Pixel {
    let rgb = [pixel[0], pixel[1], pixel[2]].map(|v| f64::from(v) / 255.0);
    let max = rgb.into_iter().fold(0.0_f64, f64::max);
    let min = rgb.into_iter().fold(1.0_f64, f64::min);
    let chroma = max - min;
    let mut l = (max + min) * 0.5;
    let (h, s) = if chroma == 0.0 {
        (0.0, 0.0)
    } else {
        let h = if max == rgb[0] {
            ((rgb[1] - rgb[2]) / chroma).rem_euclid(6.0)
        } else if max == rgb[1] {
            (rgb[2] - rgb[0]) / chroma + 2.0
        } else {
            (rgb[0] - rgb[1]) / chroma + 4.0
        };
        (h, chroma / (1.0 - (2.0 * l - 1.0).abs()))
    };
    let h = (h + f64::from(hue.clamp(-180, 180)) / 60.0).rem_euclid(6.0);
    let s = (s * f64::from(saturation.clamp(0.0, 2.0))).clamp(0.0, 1.0);
    let shift = f64::from(lightness.clamp(-100, 100)) / 100.0;
    l = if shift < 0.0 {
        l * (1.0 + shift)
    } else {
        l + (1.0 - l) * shift
    };
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
    let rgb = match h as u8 {
        0 => [c, x, 0.0],
        1 => [x, c, 0.0],
        2 => [0.0, c, x],
        3 => [0.0, x, c],
        4 => [x, 0.0, c],
        _ => [c, 0.0, x],
    };
    let rgb = rgb.map(|v| ((v + l - c * 0.5) * 255.0).round().clamp(0.0, 255.0) as u8);
    [rgb[0], rgb[1], rgb[2], pixel[3]]
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

fn pixelate(
    source: &[Pixel],
    width: u32,
    height: u32,
    size: u32,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    let (width, height, size) = (width as usize, height as usize, size as usize);
    let mut output = vec![[0; 4]; source.len()];
    for top in (0..height).step_by(size) {
        for left in (0..width).step_by(size) {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            let bottom = (top + size).min(height);
            let right = (left + size).min(width);
            let mut sum = [0.0; 4];
            for y in top..bottom {
                for x in left..right {
                    let pixel = premultiply(source[y * width + x]);
                    for c in 0..4 {
                        sum[c] += pixel[c];
                    }
                }
            }
            let mut average = [0; 4];
            if sum[3] > 0.0 {
                for c in 0..3 {
                    average[c] = (sum[c] / sum[3]).round().clamp(0.0, 255.0) as u8;
                }
                average[3] = (sum[3] / ((bottom - top) * (right - left)) as f64)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            for y in top..bottom {
                output[y * width + left..y * width + right].fill(average);
            }
        }
    }
    Some(output)
}
