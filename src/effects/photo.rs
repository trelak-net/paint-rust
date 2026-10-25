use super::{AtomicBool, Effect, Ordering, Pixel};
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
    let radius = match effect {
        Effect::Glow { amount: 0.0, .. }
        | Effect::SoftenPortrait {
            softness: 0.0,
            warmth: 0.0,
            ..
        } => return Some(source.to_vec()),
        Effect::Glow { radius, .. } | Effect::SoftenPortrait { radius, .. } => radius,
        _ => return None,
    };
    let blurred = super::blurs::gaussian(source, width, height, radius, cancel)?;
    let mut output = source.to_vec();
    for (index, (pixel, blur)) in source.iter().zip(blurred).enumerate() {
        if index.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
            return None;
        }
        if pixel[3] == 0 {
            continue;
        }
        for c in 0..3 {
            let value = f64::from(pixel[c]);
            let soft = f64::from(blur[c]);
            let result = match effect {
                Effect::Glow { amount, .. } => {
                    let screen = 255.0 - (255.0 - value) * (255.0 - soft) / 255.0;
                    value + (screen - value) * f64::from(amount) * f64::from(blur[3]) / 255.0
                }
                Effect::SoftenPortrait {
                    softness, warmth, ..
                } => {
                    let mixed = value + (soft - value) * f64::from(softness);
                    mixed + f64::from(warmth) * [24.0, 4.0, -24.0][c]
                }
                _ => unreachable!(),
            };
            output[index][c] = result.round().clamp(0.0, 255.0) as u8;
        }
    }
    Some(output)
}

pub(super) fn straighten(
    source: &[Pixel],
    width: u32,
    height: u32,
    angle: f32,
    auto_crop: bool,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    if !angle.is_finite() || !(-45.0..=45.0).contains(&angle) {
        return None;
    }
    if angle == 0.0 {
        return Some(source.to_vec());
    }
    if !auto_crop {
        return crate::affine::render(
            source,
            width,
            height,
            None,
            crate::affine::FreeTransform {
                angle,
                ..Default::default()
            },
            cancel,
        );
    }
    let (sin, cos) = f64::from(angle).to_radians().sin_cos();
    let (w, h) = (f64::from(width - 1), f64::from(height - 1));
    let scale = ((cos.abs() * w + sin.abs() * h) / w.max(1.0))
        .max((sin.abs() * w + cos.abs() * h) / h.max(1.0))
        .max(1.0);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let mut output = vec![[0; 4]; source.len()];
    for y in 0..height {
        for x in 0..width {
            if x.is_multiple_of(256) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let (dx, dy) = (f64::from(x) - cx, f64::from(y) - cy);
            let (sx, sy) = (
                cx + (cos * dx + sin * dy) / scale,
                cy + (-sin * dx + cos * dy) / scale,
            );
            output[y as usize * width as usize + x as usize] =
                super::distort::sample(source, width, height, sx, sy);
        }
    }
    Some(output)
}
