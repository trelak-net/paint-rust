use crate::document::{GradientStyle, Pixel, Selection, blend_layer};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GradientShape {
    Linear,
    Reflected,
    Radial,
    Diamond,
    Conical,
}
impl GradientShape {
    pub const ALL: [Self; 5] = [
        Self::Linear,
        Self::Reflected,
        Self::Radial,
        Self::Diamond,
        Self::Conical,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Reflected => "Reflected",
            Self::Radial => "Radial",
            Self::Diamond => "Diamond",
            Self::Conical => "Conical",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GradientMode {
    Blend,
    Replace,
    Transparency,
}
impl GradientMode {
    pub const ALL: [Self; 3] = [Self::Blend, Self::Replace, Self::Transparency];
    pub fn label(self) -> &'static str {
        match self {
            Self::Blend => "Blend colors",
            Self::Replace => "Replace colors",
            Self::Transparency => "Transparency",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gradient {
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub style: GradientStyle,
}

pub fn render(
    source: &[Pixel],
    width: u32,
    height: u32,
    selection: Option<&Selection>,
    parameters: Gradient,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    let Gradient { from, to, style } = parameters;
    if cancel.load(Ordering::Relaxed)
        || width == 0
        || height == 0
        || u64::from(width) * u64::from(height) != source.len() as u64
        || [from.0, from.1, to.0, to.1].iter().any(|v| !v.is_finite())
    {
        return None;
    }
    let mut output = source.to_vec();
    let (dx, dy) = (
        f64::from(to.0) - f64::from(from.0),
        f64::from(to.1) - f64::from(from.1),
    );
    let distance = dx * dx + dy * dy;
    for y in 0..height {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        for x in 0..width {
            if x.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            if !selection.is_none_or(|s| s.contains(x as usize, y as usize)) {
                continue;
            }
            let (px, py) = (
                f64::from(x) + 0.5 - f64::from(from.0),
                f64::from(y) + 0.5 - f64::from(from.1),
            );
            let t = if distance <= f64::EPSILON {
                0.0
            } else {
                let along = (px * dx + py * dy) / distance;
                let across = (py * dx - px * dy) / distance;
                match style.shape {
                    GradientShape::Linear => along,
                    GradientShape::Reflected => along.abs(),
                    GradientShape::Radial => (px * px + py * py).sqrt() / distance.sqrt(),
                    GradientShape::Diamond => along.abs() + across.abs(),
                    GradientShape::Conical => {
                        across.atan2(along).rem_euclid(std::f64::consts::TAU)
                            / std::f64::consts::TAU
                    }
                }
            };
            let t = t.clamp(0.0, 1.0);
            let a = f64::from(style.start[3]) * (1.0 - t);
            let b = f64::from(style.end[3]) * t;
            let mut color = [0; 4];
            let alpha = a + b;
            if alpha > 0.0 {
                for (c, channel) in color[..3].iter_mut().enumerate() {
                    *channel = ((f64::from(style.start[c]) * a + f64::from(style.end[c]) * b)
                        / alpha)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                }
                color[3] = alpha.round().clamp(0.0, 255.0) as u8;
            }
            let i = y as usize * width as usize + x as usize;
            match style.mode {
                GradientMode::Blend => {
                    output[i] = blend_layer(output[i], color, 255, style.blend_mode)
                }
                GradientMode::Replace => output[i] = color,
                GradientMode::Transparency => {
                    output[i][3] =
                        ((u16::from(source[i][3]) * u16::from(color[3]) + 127) / 255) as u8
                }
            }
        }
    }
    if cancel.load(Ordering::Relaxed) {
        None
    } else {
        Some(output)
    }
}
