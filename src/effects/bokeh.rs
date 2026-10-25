use super::{AtomicBool, Ordering, Pixel};
pub(super) fn render(
    source: &[Pixel],
    width: u32,
    height: u32,
    radius: f32,
    gamma: f32,
    quality: u8,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    if !radius.is_finite()
        || !(0.0..=100.0).contains(&radius)
        || !gamma.is_finite()
        || !(0.1..=5.0).contains(&gamma)
        || !(1..=5).contains(&quality)
    {
        return None;
    }
    if radius == 0.0 {
        return Some(source.to_vec());
    }
    let pairs = 8usize << quality;
    let mut offsets = Vec::with_capacity(pairs * 2 + 1);
    offsets.push((0.0, 0.0));
    for index in 0..pairs {
        let distance = ((index as f64 + 0.5) / pairs as f64).sqrt() * f64::from(radius);
        let angle = index as f64 * std::f64::consts::PI * (3.0 - 5.0f64.sqrt());
        let (x, y) = (distance * angle.cos(), distance * angle.sin());
        offsets.push((x, y));
        offsets.push((-x, -y));
    }
    let table = std::array::from_fn::<f64, 256, _>(|i| (i as f64 / 255.0).powf(f64::from(gamma)));
    let mut output = vec![[0; 4]; source.len()];
    for y in 0..height {
        for x in 0..width {
            if x.is_multiple_of(16) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let mut sum = [0.0; 4];
            for &(dx, dy) in &offsets {
                let px = (f64::from(x) + dx).clamp(0.0, f64::from(width - 1));
                let py = (f64::from(y) + dy).clamp(0.0, f64::from(height - 1));
                let (x0, y0) = (px.floor() as u32, py.floor() as u32);
                let (tx, ty) = (px.fract(), py.fract());
                for (sx, sy, weight) in [
                    (x0, y0, (1.0 - tx) * (1.0 - ty)),
                    ((x0 + 1).min(width - 1), y0, tx * (1.0 - ty)),
                    (x0, (y0 + 1).min(height - 1), (1.0 - tx) * ty),
                    ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1), tx * ty),
                ] {
                    let pixel = source[sy as usize * width as usize + sx as usize];
                    let alpha = f64::from(pixel[3]) * weight;
                    sum[3] += alpha;
                    for c in 0..3 {
                        sum[c] += table[pixel[c] as usize] * alpha;
                    }
                }
            }
            if sum[3] > 0.0 {
                let dest = &mut output[y as usize * width as usize + x as usize];
                for c in 0..3 {
                    dest[c] = ((sum[c] / sum[3])
                        .clamp(0.0, 1.0)
                        .powf(1.0 / f64::from(gamma))
                        * 255.0)
                        .round() as u8;
                }
                dest[3] = (sum[3] / offsets.len() as f64).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Some(output)
}
