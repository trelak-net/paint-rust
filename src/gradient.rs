use crate::document::{GradientStyle, Pixel, Selection, blend};
use std::sync::atomic::{AtomicBool, Ordering};

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
    let (dx, dy) = (f64::from(to.0 - from.0), f64::from(to.1 - from.1));
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
            } else if style.radial {
                (px * px + py * py).sqrt() / distance.sqrt()
            } else {
                (px * dx + py * dy) / distance
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
            output[i] = blend(output[i], color, 255);
        }
    }
    if cancel.load(Ordering::Relaxed) {
        None
    } else {
        Some(output)
    }
}
