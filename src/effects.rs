mod blurs;
mod detail;
mod distort;
mod morphology;
mod object;
pub use morphology::MorphologyOperation;
mod photo;
mod render;
pub use render::{FractalSettings, NoiseSettings};
mod stylize;
use crate::document::{Pixel, Selection};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Effect {
    ReduceNoise {
        radius: u32,
        strength: f32,
    },
    Relief {
        angle: f32,
        strength: f32,
    },
    Outline {
        radius: u32,
        intensity: f32,
    },
    Morphology {
        radius: u32,
        operation: MorphologyOperation,
    },
    Dents {
        scale: f32,
        amount: f32,
        seed: u32,
    },
    PolarInversion {
        amount: f32,
        center: [f32; 2],
    },
    TileReflection {
        size: f32,
        amount: f32,
        angle: f32,
    },
    Clouds(NoiseSettings),
    Turbulence(NoiseSettings),
    Mandelbrot(FractalSettings),
    Julia(FractalSettings),
    Glow {
        radius: u32,
        amount: f32,
    },
    SoftenPortrait {
        radius: u32,
        softness: f32,
        warmth: f32,
    },
    DropShadow {
        radius: u32,
        offset: [i32; 2],
        color: Pixel,
    },
    SurfaceBlur {
        radius: u32,
        threshold: u8,
    },
    RadialBlur {
        angle: f32,
        center: [f32; 2],
    },
    ZoomBlur {
        amount: f32,
        center: [f32; 2],
    },
    FragmentBlur {
        distance: f32,
        fragments: u8,
        angle: f32,
    },
    Exposure {
        stops: f32,
        offset: f32,
        gamma: f32,
    },
    Bulge(f32),
    Twist(f32),
    FrostedGlass {
        radius: u32,
        seed: u32,
    },
    Crystalize {
        size: u32,
        seed: u32,
    },
    Vignette {
        amount: f32,
        roundness: f32,
    },
    RedEye(f32),
    AddNoise {
        amount: u8,
        coverage: u8,
        monochrome: bool,
        seed: u32,
    },
    Median {
        radius: u32,
        percentile: u8,
    },
    EdgeDetect,
    Emboss(f32),
    InvertAlpha,
    TemperatureTint {
        temperature: i16,
        tint: i16,
    },
    HighlightsShadows {
        highlights: i16,
        shadows: i16,
    },
    GaussianBlur(u32),
    MotionBlur {
        distance: u32,
        angle: f32,
        centered: bool,
    },
    Curves(crate::curves::Curves),
    Levels(crate::levels::Levels),
    AutoLevels(f32),
    #[serde(skip)]
    Move(crate::movement::Parameters),
    #[serde(skip)]
    Shape(crate::shapes::Parameters),
    #[serde(skip)]
    Gradient(crate::gradient::Gradient),
    #[serde(skip)]
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
    pub fn valid_settings(self) -> bool {
        let range = |v: f32, min: f32, max: f32| v.is_finite() && (min..=max).contains(&v);
        match self {
            Self::Move(_) | Self::Shape(_) | Self::Gradient(_) | Self::FreeTransform(_) => false,
            Self::ReduceNoise { radius, strength } => radius <= 10 && range(strength, 0.0, 1.0),
            Self::Relief { angle, strength } => {
                range(angle, -180.0, 180.0) && range(strength, 0.0, 3.0)
            }
            Self::Outline { radius, intensity } => {
                (1..=10).contains(&radius) && range(intensity, 0.0, 4.0)
            }
            Self::Morphology { radius, .. } => radius <= 64,
            Self::Dents { scale, amount, .. } => {
                range(scale, 1.0, 1000.0) && range(amount, 0.0, 200.0)
            }
            Self::PolarInversion { amount, center } => {
                range(amount, -1.0, 1.0) && center.into_iter().all(|v| range(v, 0.0, 1.0))
            }
            Self::TileReflection {
                size,
                amount,
                angle,
            } => {
                range(size, 1.0, 500.0)
                    && range(amount, -100.0, 100.0)
                    && range(angle, -180.0, 180.0)
            }
            Self::Clouds(settings) | Self::Turbulence(settings) => settings.valid(),
            Self::Mandelbrot(settings) | Self::Julia(settings) => settings.valid(),
            Self::Levels(levels) => levels.tables().is_some(),
            Self::Curves(_)
            | Self::InvertAlpha
            | Self::Grayscale
            | Self::Sepia
            | Self::EdgeDetect => true,
            Self::Exposure {
                stops,
                offset,
                gamma,
            } => range(stops, -10.0, 10.0) && range(offset, -1.0, 1.0) && range(gamma, 0.1, 10.0),
            Self::RadialBlur { angle, center } => {
                range(angle, 0.0, 180.0) && center.into_iter().all(|v| range(v, 0.0, 1.0))
            }
            Self::ZoomBlur { amount, center } => {
                range(amount, 0.0, 1.0) && center.into_iter().all(|v| range(v, 0.0, 1.0))
            }
            Self::FragmentBlur {
                distance,
                fragments,
                angle,
            } => {
                range(distance, 0.0, 200.0)
                    && (2..=32).contains(&fragments)
                    && range(angle, -180.0, 180.0)
            }
            Self::SurfaceBlur { radius, threshold } => radius <= 10 && threshold > 0,
            Self::DropShadow { radius, offset, .. } => {
                radius <= 64 && offset.into_iter().all(|v| (-200..=200).contains(&v))
            }
            Self::Glow { radius, amount } => radius <= 64 && range(amount, 0.0, 1.0),
            Self::SoftenPortrait {
                radius,
                softness,
                warmth,
            } => radius <= 64 && range(softness, 0.0, 1.0) && range(warmth, -1.0, 1.0),
            Self::AutoLevels(v) => range(v, 0.0, 5.0),
            Self::BrightnessContrast {
                brightness,
                contrast,
            } => (-100..=100).contains(&brightness) && (-100..=100).contains(&contrast),
            Self::TemperatureTint {
                temperature: a,
                tint: b,
            }
            | Self::HighlightsShadows {
                highlights: a,
                shadows: b,
            } => (-100..=100).contains(&a) && (-100..=100).contains(&b),
            Self::Saturation(v) => range(v, 0.0, 2.0),
            Self::HueSaturation {
                hue,
                saturation,
                lightness,
            } => {
                (-180..=180).contains(&hue)
                    && range(saturation, 0.0, 2.0)
                    && (-100..=100).contains(&lightness)
            }
            Self::BoxBlur(r) | Self::GaussianBlur(r) => r <= 64,
            Self::MotionBlur {
                distance, angle, ..
            } => distance <= 200 && range(angle, -180.0, 180.0),
            Self::Pixelate(size) => (1..=128).contains(&size),
            Self::Posterize(levels) => (2..=32).contains(&levels),
            Self::Sharpen(v) => range(v, 0.0, 3.0),
            Self::AddNoise {
                amount, coverage, ..
            } => amount <= 100 && coverage <= 100,
            Self::Median { radius, percentile } => radius <= 10 && percentile <= 100,
            Self::Emboss(v) => range(v, -180.0, 180.0),
            Self::Bulge(v) => range(v, -100.0, 100.0),
            Self::Twist(v) => range(v, -720.0, 720.0),
            Self::FrostedGlass { radius, .. } => radius <= 100,
            Self::Crystalize { size, .. } => (1..=200).contains(&size),
            Self::Vignette { amount, roundness } => {
                range(amount, 0.0, 1.0) && range(roundness, 0.0, 1.0)
            }
            Self::RedEye(v) => range(v, 1.0, 3.0),
        }
    }

    pub fn category(self) -> &'static str {
        match self {
            Self::Clouds(_) | Self::Turbulence(_) | Self::Mandelbrot(_) | Self::Julia(_) => {
                "Render"
            }
            Self::SurfaceBlur { .. }
            | Self::RadialBlur { .. }
            | Self::ZoomBlur { .. }
            | Self::FragmentBlur { .. }
            | Self::GaussianBlur(_)
            | Self::BoxBlur(_)
            | Self::MotionBlur { .. }
            | Self::Median { .. } => "Blurs",
            Self::Morphology { .. }
            | Self::Dents { .. }
            | Self::PolarInversion { .. }
            | Self::TileReflection { .. }
            | Self::Bulge(_)
            | Self::Twist(_)
            | Self::FrostedGlass { .. }
            | Self::Crystalize { .. }
            | Self::Pixelate(_) => "Distort",
            Self::DropShadow { .. } => "Object",
            Self::AddNoise { .. } | Self::ReduceNoise { .. } => "Noise",
            Self::Glow { .. }
            | Self::SoftenPortrait { .. }
            | Self::RedEye(_)
            | Self::Vignette { .. }
            | Self::Sharpen(_) => "Photo",
            Self::EdgeDetect | Self::Emboss(_) | Self::Outline { .. } | Self::Relief { .. } => {
                "Stylize"
            }
            Self::Move(_) | Self::Shape(_) | Self::Gradient(_) | Self::FreeTransform(_) => {
                "Geometry"
            }
            _ => "Adjustments",
        }
    }

    pub fn repeatable(self) -> bool {
        !matches!(
            self,
            Self::Move(_) | Self::Shape(_) | Self::Gradient(_) | Self::FreeTransform(_)
        )
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::ReduceNoise { .. } => "Reduce noise",
            Self::Relief { .. } => "Relief",
            Self::Outline { .. } => "Outline",
            Self::Morphology { .. } => "Morphology",
            Self::Dents { .. } => "Dents",
            Self::PolarInversion { .. } => "Polar inversion",
            Self::TileReflection { .. } => "Tile reflection",
            Self::Clouds(_) => "Clouds",
            Self::Turbulence(_) => "Turbulence",
            Self::Mandelbrot(_) => "Mandelbrot fractal",
            Self::Julia(_) => "Julia fractal",
            Self::RadialBlur { .. } => "Radial blur",
            Self::ZoomBlur { .. } => "Zoom blur",
            Self::FragmentBlur { .. } => "Fragment blur",
            Self::SurfaceBlur { .. } => "Surface blur",
            Self::DropShadow { .. } => "Drop shadow",
            Self::Glow { .. } => "Glow",
            Self::SoftenPortrait { .. } => "Soften portrait",
            Self::Exposure { .. } => "Exposure",
            Self::Bulge(_) => "Bulge",
            Self::Twist(_) => "Twist",
            Self::FrostedGlass { .. } => "Frosted glass",
            Self::Crystalize { .. } => "Crystalize",
            Self::Vignette { .. } => "Vignette",
            Self::RedEye(_) => "Red eye removal",
            Self::AddNoise { .. } => "Add noise",
            Self::Median { .. } => "Median blur",
            Self::EdgeDetect => "Edge detect",
            Self::Emboss(_) => "Emboss",
            Self::InvertAlpha => "Invert alpha",
            Self::TemperatureTint { .. } => "Temperature / Tint",
            Self::HighlightsShadows { .. } => "Highlights / Shadows",
            Self::GaussianBlur(_) => "Gaussian blur",
            Self::MotionBlur { .. } => "Motion blur",
            Self::Curves(_) => "Curves",
            Self::Levels(_) => "Levels",
            Self::AutoLevels(_) => "Auto levels",
            Self::Move(parameters) => {
                if parameters.copy {
                    "Copy pixels"
                } else {
                    "Move pixels"
                }
            }
            Self::Shape(parameters) => parameters.shape.label(),
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
    let exposure_table = if let Effect::Exposure {
        stops,
        offset,
        gamma,
    } = effect
    {
        if !effect.valid_settings() {
            return None;
        }
        Some(std::array::from_fn::<u8, 256, _>(|i| {
            if stops == 0.0 && offset == 0.0 && gamma == 1.0 {
                return i as u8;
            }
            let encoded = i as f64 / 255.0;
            let linear = if encoded <= 0.04045 {
                encoded / 12.92
            } else {
                ((encoded + 0.055) / 1.055).powf(2.4)
            };
            let adjusted = (linear * 2.0_f64.powf(f64::from(stops)) + f64::from(offset))
                .clamp(0.0, 1.0)
                .powf(1.0 / f64::from(gamma));
            let encoded = if adjusted <= 0.0031308 {
                adjusted * 12.92
            } else {
                1.055 * adjusted.powf(1.0 / 2.4) - 0.055
            };
            (encoded * 255.0).round().clamp(0.0, 255.0) as u8
        }))
    } else {
        None
    };
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
    if matches!(effect,Effect::Vignette {amount,roundness} if !amount.is_finite() || !roundness.is_finite())
        || matches!(effect,Effect::RedEye(v) if !v.is_finite())
    {
        return None;
    }
    let luminance_table = match effect {
        Effect::Curves(parameters) if parameters.luminance => {
            Some(parameters.luminance_curve.table())
        }
        _ => None,
    };
    let level_tables = match effect {
        Effect::Curves(parameters) if !parameters.luminance => Some(parameters.tables()),
        Effect::Levels(parameters) => Some(parameters.tables()?),
        Effect::AutoLevels(clip) => Some(
            crate::levels::automatic(source, width, selection.as_ref(), clip, cancel)?.tables()?,
        ),
        _ => None,
    };
    if matches!(
        effect,
        Effect::Dents { .. } | Effect::PolarInversion { .. } | Effect::TileReflection { .. }
    ) && !effect.valid_settings()
    {
        return None;
    }
    let mut output = match effect {
        Effect::ReduceNoise { .. } | Effect::Relief { .. } | Effect::Outline { .. } => {
            detail::render(source, width, height, effect, cancel)?
        }
        Effect::Morphology { radius, operation } => {
            morphology::render(source, width, height, radius, operation, cancel)?
        }
        Effect::Dents {
            scale,
            amount,
            seed,
        } => distort::render(
            source,
            width,
            height,
            distort::Warp::Dents {
                scale,
                amount,
                seed,
            },
            cancel,
        )?,
        Effect::PolarInversion { amount, center } => distort::render(
            source,
            width,
            height,
            distort::Warp::Polar { amount, center },
            cancel,
        )?,
        Effect::TileReflection {
            size,
            amount,
            angle,
        } => distort::render(
            source,
            width,
            height,
            distort::Warp::Tiles {
                size,
                amount,
                angle,
            },
            cancel,
        )?,
        Effect::Clouds(_) | Effect::Turbulence(_) | Effect::Mandelbrot(_) | Effect::Julia(_) => {
            render::render(width, height, effect, cancel)?
        }
        Effect::Bulge(amount) => {
            distort::render(source, width, height, distort::Warp::Bulge(amount), cancel)?
        }
        Effect::Twist(angle) => {
            distort::render(source, width, height, distort::Warp::Twist(angle), cancel)?
        }
        Effect::FrostedGlass { radius, seed } => distort::render(
            source,
            width,
            height,
            distort::Warp::Frosted { radius, seed },
            cancel,
        )?,
        Effect::Crystalize { size, seed } => distort::render(
            source,
            width,
            height,
            distort::Warp::Crystal { size, seed },
            cancel,
        )?,
        Effect::Median { radius, percentile } => {
            stylize::median(source, width, height, radius, percentile, cancel)?
        }
        Effect::EdgeDetect => stylize::edges(source, width, height, None, cancel)?,
        Effect::Emboss(angle) => stylize::edges(source, width, height, Some(angle), cancel)?,
        Effect::RadialBlur { .. } | Effect::ZoomBlur { .. } | Effect::FragmentBlur { .. } => {
            blurs::trajectory(source, width, height, effect, cancel)?
        }
        Effect::SurfaceBlur { radius, threshold } => {
            blurs::surface(source, width, height, radius, threshold, cancel)?
        }
        Effect::DropShadow {
            radius,
            offset,
            color,
        } => object::shadow(source, width, height, radius, offset, color, cancel)?,
        Effect::Glow { .. } | Effect::SoftenPortrait { .. } => {
            photo::render(source, width, height, effect, cancel)?
        }
        Effect::GaussianBlur(radius) => blurs::gaussian(source, width, height, radius, cancel)?,
        Effect::MotionBlur {
            distance,
            angle,
            centered,
        } => blurs::motion(source, width, height, distance, angle, centered, cancel)?,
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
            if let Some(table) = &exposure_table {
                if p[3] > 0 {
                    for c in 0..3 {
                        output[i][c] = table[p[c] as usize];
                    }
                }
                continue;
            }
            if let Effect::Vignette { amount, roundness } = effect {
                let dx = (x as f64 + 0.5 - f64::from(width) / 2.0) / (f64::from(width) / 2.0);
                let dy = (y as f64 + 0.5 - f64::from(height) / 2.0) / (f64::from(height) / 2.0);
                let radius = f64::from(roundness.clamp(0.0, 1.0));
                let falloff = ((dx.hypot(dy) - radius) / (std::f64::consts::SQRT_2 - radius))
                    .clamp(0.0, 1.0)
                    .powi(2);
                let gain = 1.0 - f64::from(amount.clamp(0.0, 1.0)) * falloff;
                if p[3] > 0 {
                    for (c, channel) in p[..3].iter().enumerate() {
                        output[i][c] = (f64::from(*channel) * gain).round() as u8;
                    }
                }
                continue;
            }
            if let Effect::RedEye(threshold) = effect {
                if p[3] > 0
                    && p[0] > 40
                    && f32::from(p[0])
                        > f32::from(p[1].max(p[2]).saturating_add(1)) * threshold.clamp(1.0, 3.0)
                {
                    output[i][0] = ((u16::from(p[1]) + u16::from(p[2])) / 2) as u8;
                }
                continue;
            }
            if let Effect::AddNoise {
                amount,
                coverage,
                monochrome,
                seed,
            } = effect
            {
                output[i] = stylize::noise(p, i, amount, coverage, monochrome, seed);
                continue;
            }
            if effect == Effect::InvertAlpha {
                output[i][3] = 255 - p[3];
                continue;
            }
            if let Some(table) = &luminance_table {
                if p[3] > 0 {
                    let luma = (0.2126 * f64::from(p[0])
                        + 0.7152 * f64::from(p[1])
                        + 0.0722 * f64::from(p[2]))
                    .round() as u8;
                    let delta = i16::from(table[luma as usize]) - i16::from(luma);
                    for c in 0..3 {
                        output[i][c] = (i16::from(p[c]) + delta).clamp(0, 255) as u8;
                    }
                }
                continue;
            }
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
                    Effect::TemperatureTint { temperature, tint } => {
                        let warm = f64::from(temperature.clamp(-100, 100)) * 0.64;
                        let green = f64::from(tint.clamp(-100, 100)) * 0.64;
                        f64::from(*channel) + [warm + green * 0.5, -green, -warm + green * 0.5][c]
                    }
                    Effect::HighlightsShadows {
                        highlights,
                        shadows,
                    } => {
                        let l = luminance / 255.0;
                        let shadow_weight = (1.0 - l).powi(2);
                        let highlight_weight = l.powi(2);
                        let amount = (f64::from(shadows.clamp(-100, 100)) * shadow_weight
                            + f64::from(highlights.clamp(-100, 100)) * highlight_weight)
                            / 100.0;
                        let value = f64::from(*channel);
                        if amount >= 0.0 {
                            value + (255.0 - value) * amount
                        } else {
                            value * (1.0 + amount)
                        }
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
                    Effect::ReduceNoise { .. }
                    | Effect::Relief { .. }
                    | Effect::Outline { .. }
                    | Effect::Morphology { .. }
                    | Effect::Dents { .. }
                    | Effect::PolarInversion { .. }
                    | Effect::TileReflection { .. }
                    | Effect::Clouds(_)
                    | Effect::Turbulence(_)
                    | Effect::Mandelbrot(_)
                    | Effect::Julia(_)
                    | Effect::Glow { .. }
                    | Effect::SoftenPortrait { .. }
                    | Effect::DropShadow { .. }
                    | Effect::SurfaceBlur { .. }
                    | Effect::RadialBlur { .. }
                    | Effect::ZoomBlur { .. }
                    | Effect::FragmentBlur { .. }
                    | Effect::Exposure { .. }
                    | Effect::Bulge(_)
                    | Effect::Twist(_)
                    | Effect::FrostedGlass { .. }
                    | Effect::Crystalize { .. }
                    | Effect::Vignette { .. }
                    | Effect::RedEye(_)
                    | Effect::AddNoise { .. }
                    | Effect::Median { .. }
                    | Effect::EdgeDetect
                    | Effect::Emboss(_)
                    | Effect::InvertAlpha
                    | Effect::GaussianBlur(_)
                    | Effect::MotionBlur { .. }
                    | Effect::Curves(_)
                    | Effect::Levels(_)
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
