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
