use super::{AtomicBool, Ordering, Pixel, premultiply};

fn unpremultiply(sum: [f64; 4]) -> Pixel {
    if sum[3] <= 0.0 {
        return [0; 4];
    }
    [
        (sum[0] / sum[3]).round().clamp(0.0, 255.0) as u8,
        (sum[1] / sum[3]).round().clamp(0.0, 255.0) as u8,
        (sum[2] / sum[3]).round().clamp(0.0, 255.0) as u8,
        sum[3].round().clamp(0.0, 255.0) as u8,
    ]
}

pub(super) fn gaussian(
    source: &[Pixel],
    width: u32,
    height: u32,
    radius: u32,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    let radius = radius.min(64) as i64;
    if radius == 0 {
        return Some(source.to_vec());
    }
    let sigma = (radius as f64 / 3.0).max(0.5);
    let mut weights: Vec<_> = (-radius..=radius)
        .map(|x| (-(x * x) as f64 / (2.0 * sigma * sigma)).exp())
        .collect();
    let total: f64 = weights.iter().sum();
    for weight in &mut weights {
        *weight /= total;
    }
    let (w, h) = (width as usize, height as usize);
    let mut horizontal = vec![[0.0_f32; 4]; source.len()];
    for y in 0..h {
        for x in 0..w {
            if x.is_multiple_of(256) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let mut sum = [0.0; 4];
            for (index, weight) in weights.iter().enumerate() {
                let sx = (x as i64 + index as i64 - radius).clamp(0, w as i64 - 1) as usize;
                let sample = premultiply(source[y * w + sx]);
                for c in 0..4 {
                    sum[c] += sample[c] * weight;
                }
            }
            horizontal[y * w + x] = sum.map(|v| v as f32);
        }
    }
    let mut output = vec![[0; 4]; source.len()];
    for y in 0..h {
        for x in 0..w {
            if x.is_multiple_of(256) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let mut sum = [0.0; 4];
            for (index, weight) in weights.iter().enumerate() {
                let sy = (y as i64 + index as i64 - radius).clamp(0, h as i64 - 1) as usize;
                for (c, value) in sum.iter_mut().enumerate() {
                    *value += f64::from(horizontal[sy * w + x][c]) * weight;
                }
            }
            output[y * w + x] = unpremultiply(sum);
        }
    }
    Some(output)
}

pub(super) fn motion(
    source: &[Pixel],
    width: u32,
    height: u32,
    distance: u32,
    angle: f32,
    centered: bool,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    if !angle.is_finite() {
        return None;
    }
    let distance = distance.min(200);
    if distance == 0 {
        return Some(source.to_vec());
    }
    let angle = f64::from(angle).to_radians();
    let count = distance as usize * 2 + 1;
    let offsets: Vec<_> = (0..count)
        .map(|i| {
            let t = i as f64 / (count - 1) as f64 - if centered { 0.5 } else { 0.0 };
            (
                t * f64::from(distance) * angle.cos(),
                t * f64::from(distance) * angle.sin(),
            )
        })
        .collect();
    let mut output = vec![[0; 4]; source.len()];
    for y in 0..height {
        for x in 0..width {
            if x.is_multiple_of(64) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let mut sum = [0.0; 4];
            for &(dx, dy) in &offsets {
                let sx = (f64::from(x) + dx).clamp(0.0, f64::from(width - 1));
                let sy = (f64::from(y) + dy).clamp(0.0, f64::from(height - 1));
                let (x0, y0) = (sx.floor() as u32, sy.floor() as u32);
                let (tx, ty) = (sx - f64::from(x0), sy - f64::from(y0));
                for (ix, iy, weight) in [
                    (x0, y0, (1.0 - tx) * (1.0 - ty)),
                    ((x0 + 1).min(width - 1), y0, tx * (1.0 - ty)),
                    (x0, (y0 + 1).min(height - 1), (1.0 - tx) * ty),
                    ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1), tx * ty),
                ] {
                    let pixel = premultiply(source[iy as usize * width as usize + ix as usize]);
                    for c in 0..4 {
                        sum[c] += pixel[c] * weight;
                    }
                }
            }
            output[y as usize * width as usize + x as usize] =
                unpremultiply(sum.map(|v| v / count as f64));
        }
    }
    Some(output)
}

fn bilinear(source: &[Pixel], width: u32, height: u32, x: f64, y: f64) -> [f64; 4] {
    let x = x.clamp(0.0, f64::from(width - 1));
    let y = y.clamp(0.0, f64::from(height - 1));
    let (x0, y0) = (x.floor() as u32, y.floor() as u32);
    let (tx, ty) = (x - f64::from(x0), y - f64::from(y0));
    let mut sum = [0.0; 4];
    for (sx, sy, weight) in [
        (x0, y0, (1.0 - tx) * (1.0 - ty)),
        ((x0 + 1).min(width - 1), y0, tx * (1.0 - ty)),
        (x0, (y0 + 1).min(height - 1), (1.0 - tx) * ty),
        ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1), tx * ty),
    ] {
        let pixel = premultiply(source[(sy * width + sx) as usize]);
        for c in 0..4 {
            sum[c] += pixel[c] * weight;
        }
    }
    sum
}

pub(super) fn trajectory(
    source: &[Pixel],
    width: u32,
    height: u32,
    effect: super::Effect,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    use super::Effect;
    if !effect.valid_settings() {
        return None;
    }
    if matches!(
        effect,
        Effect::RadialBlur { angle: 0.0, .. }
            | Effect::ZoomBlur { amount: 0.0, .. }
            | Effect::FragmentBlur { distance: 0.0, .. }
    ) {
        return Some(source.to_vec());
    }
    let (count, center) = match effect {
        Effect::RadialBlur { center, .. } | Effect::ZoomBlur { center, .. } => (65, center),
        Effect::FragmentBlur { fragments, .. } => (usize::from(fragments), [0.5, 0.5]),
        _ => return None,
    };
    let (cx, cy) = (
        f64::from(center[0]) * f64::from(width - 1),
        f64::from(center[1]) * f64::from(height - 1),
    );
    // Precompute the bounded sampling paths; no trigonometry in the pixel loop.
    let paths: Vec<_> = (0..count)
        .map(|i| match effect {
            Effect::RadialBlur { angle, .. } => {
                let theta = (i as f64 / (count - 1) as f64 - 0.5) * f64::from(angle).to_radians();
                (theta.cos(), theta.sin())
            }
            Effect::ZoomBlur { amount, .. } => {
                (1.0 - f64::from(amount) * i as f64 / (count - 1) as f64, 0.0)
            }
            Effect::FragmentBlur {
                distance, angle, ..
            } => {
                let theta =
                    f64::from(angle).to_radians() + std::f64::consts::TAU * i as f64 / count as f64;
                (
                    f64::from(distance) * theta.cos(),
                    f64::from(distance) * theta.sin(),
                )
            }
            _ => unreachable!(),
        })
        .collect();
    let mut output = vec![[0; 4]; source.len()];
    for y in 0..height {
        for x in 0..width {
            if x.is_multiple_of(32) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let (dx, dy) = (f64::from(x) - cx, f64::from(y) - cy);
            let mut sum = [0.0; 4];
            for &(a, b) in &paths {
                let (sx, sy) = match effect {
                    Effect::RadialBlur { .. } => (cx + dx * a - dy * b, cy + dx * b + dy * a),
                    Effect::ZoomBlur { .. } => (cx + dx * a, cy + dy * a),
                    _ => (f64::from(x) + a, f64::from(y) + b),
                };
                let pixel = bilinear(source, width, height, sx, sy);
                for c in 0..4 {
                    sum[c] += pixel[c];
                }
            }
            output[(y * width + x) as usize] = unpremultiply(sum.map(|v| v / count as f64));
        }
    }
    Some(output)
}

pub(super) fn surface(
    source: &[Pixel],
    width: u32,
    height: u32,
    radius: u32,
    threshold: u8,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    if radius > 10 || threshold == 0 {
        return None;
    }
    if radius == 0 {
        return Some(source.to_vec());
    }
    let radius = i64::from(radius);
    let sigma = (radius as f64 / 2.0).max(0.5);
    let weights: Vec<_> = (-radius..=radius)
        .flat_map(|dy| {
            (-radius..=radius).map(move |dx| {
                (
                    dx,
                    dy,
                    (-(dx * dx + dy * dy) as f64 / (2.0 * sigma * sigma)).exp(),
                )
            })
        })
        .collect();
    let mut output = source.to_vec();
    for y in 0..height {
        for x in 0..width {
            if x.is_multiple_of(16) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let index = (y * width + x) as usize;
            let center = source[index];
            if center[3] == 0 {
                continue;
            }
            let mut sum = [0.0; 3];
            let mut total = 0.0;
            for &(dx, dy, spatial) in &weights {
                let sx = (i64::from(x) + dx).clamp(0, i64::from(width) - 1) as u32;
                let sy = (i64::from(y) + dy).clamp(0, i64::from(height) - 1) as u32;
                let pixel = source[(sy * width + sx) as usize];
                let difference = (0..3).map(|c| center[c].abs_diff(pixel[c])).max().unwrap();
                if difference >= threshold || pixel[3] == 0 {
                    continue;
                }
                let weight = spatial
                    * (1.0 - f64::from(difference) / f64::from(threshold))
                    * f64::from(pixel[3]);
                for c in 0..3 {
                    sum[c] += f64::from(pixel[c]) * weight;
                }
                total += weight;
            }
            if total > 0.0 {
                for c in 0..3 {
                    output[index][c] = (sum[c] / total).round() as u8;
                }
            }
        }
    }
    Some(output)
}
