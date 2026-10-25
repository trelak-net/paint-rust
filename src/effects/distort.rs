use super::{AtomicBool, Ordering, Pixel, premultiply};
#[derive(Clone, Copy)]
pub(super) enum Warp {
    Dents { scale: f32, amount: f32, seed: u32 },
    Polar { amount: f32, center: [f32; 2] },
    Tiles { size: f32, amount: f32, angle: f32 },
    Bulge(f32),
    Twist(f32),
    Frosted { radius: u32, seed: u32 },
    Crystal { size: u32, seed: u32 },
}
fn random(x: i64, y: i64, seed: u32) -> f64 {
    let mut n = (x as u32).wrapping_mul(0x9e3779b9) ^ (y as u32).wrapping_mul(0x85ebca6b) ^ seed;
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846ca68b);
    n ^= n >> 16;
    f64::from(n) / f64::from(u32::MAX)
}
fn sample(source: &[Pixel], width: u32, height: u32, x: f64, y: f64) -> Pixel {
    let x = x.clamp(0.0, f64::from(width - 1));
    let y = y.clamp(0.0, f64::from(height - 1));
    let (x0, y0) = (x.floor() as u32, y.floor() as u32);
    let (tx, ty) = (x - f64::from(x0), y - f64::from(y0));
    if tx == 0.0 && ty == 0.0 {
        return source[y0 as usize * width as usize + x0 as usize];
    }
    let mut sum = [0.0; 4];
    for (sx, sy, weight) in [
        (x0, y0, (1.0 - tx) * (1.0 - ty)),
        ((x0 + 1).min(width - 1), y0, tx * (1.0 - ty)),
        (x0, (y0 + 1).min(height - 1), (1.0 - tx) * ty),
        ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1), tx * ty),
    ] {
        let p = premultiply(source[sy as usize * width as usize + sx as usize]);
        for c in 0..4 {
            sum[c] += p[c] * weight;
        }
    }
    if sum[3] <= 0.0 {
        return [0; 4];
    }
    [
        (sum[0] / sum[3]).round() as u8,
        (sum[1] / sum[3]).round() as u8,
        (sum[2] / sum[3]).round() as u8,
        sum[3].round() as u8,
    ]
}
pub(super) fn render(
    source: &[Pixel],
    width: u32,
    height: u32,
    warp: Warp,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    if matches!(warp,Warp::Bulge(v)|Warp::Twist(v) if !v.is_finite()) {
        return None;
    }
    if matches!(
        warp,
        Warp::Dents { amount: 0.0, .. }
            | Warp::Polar { amount: 0.0, .. }
            | Warp::Tiles { amount: 0.0, .. }
            | Warp::Bulge(0.0)
            | Warp::Twist(0.0)
            | Warp::Frosted { radius: 0, .. }
            | Warp::Crystal { size: 0..=1, .. }
    ) {
        return Some(source.to_vec());
    }
    let (cx, cy) = (f64::from(width - 1) / 2.0, f64::from(height - 1) / 2.0);
    let radius = f64::from(width.min(height)) / 2.0;
    let mut output = vec![[0; 4]; source.len()];
    for y in 0..height {
        for x in 0..width {
            if x.is_multiple_of(256) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let (px, py) = (f64::from(x), f64::from(y));
            let (sx, sy) = match warp {
                Warp::Dents {
                    scale,
                    amount,
                    seed,
                } => {
                    let nx = px / f64::from(scale);
                    let ny = py / f64::from(scale);
                    let dx = (super::render::noise(nx, ny, seed) * 2.0 - 1.0) * f64::from(amount);
                    let dy = (super::render::noise(nx, ny, seed.wrapping_add(1)) * 2.0 - 1.0)
                        * f64::from(amount);
                    (px + dx, py + dy)
                }
                Warp::Polar { amount, center } => {
                    let center_x = f64::from(center[0]) * f64::from(width - 1);
                    let center_y = f64::from(center[1]) * f64::from(height - 1);
                    let (dx, dy) = (px - center_x, py - center_y);
                    let distance = dx * dx + dy * dy;
                    if distance < 1e-12 {
                        (px, py)
                    } else {
                        let factor = 1.0 + f64::from(amount) * (radius * radius / distance - 1.0);
                        (center_x + dx * factor, center_y + dy * factor)
                    }
                }
                Warp::Tiles {
                    size,
                    amount,
                    angle,
                } => {
                    let angle = f64::from(angle).to_radians();
                    let (cos, sin) = (angle.cos(), angle.sin());
                    let (dx, dy) = (px - cx, py - cy);
                    let (tx, ty) = (dx * cos + dy * sin, -dx * sin + dy * cos);
                    let phase = std::f64::consts::TAU / f64::from(size);
                    let (tx, ty) = (
                        tx + (tx * phase).sin() * f64::from(amount),
                        ty + (ty * phase).sin() * f64::from(amount),
                    );
                    (cx + tx * cos - ty * sin, cy + tx * sin + ty * cos)
                }

                Warp::Bulge(amount) => {
                    let (dx, dy) = ((px - cx) / radius, (py - cy) / radius);
                    let r = dx.hypot(dy);
                    if r > 0.0 && r < 1.0 {
                        let factor = r.powf(
                            2.0_f64.powf(f64::from(amount.clamp(-100.0, 100.0)) / 50.0) - 1.0,
                        );
                        (cx + dx * factor * radius, cy + dy * factor * radius)
                    } else {
                        (px, py)
                    }
                }
                Warp::Twist(angle) => {
                    let (dx, dy) = (px - cx, py - cy);
                    let r = dx.hypot(dy) / radius;
                    if r < 1.0 {
                        let theta =
                            -f64::from(angle.clamp(-720.0, 720.0)).to_radians() * (1.0 - r).powi(2);
                        (
                            cx + dx * theta.cos() - dy * theta.sin(),
                            cy + dx * theta.sin() + dy * theta.cos(),
                        )
                    } else {
                        (px, py)
                    }
                }
                Warp::Frosted { radius, seed } => {
                    let distance = random(i64::from(x), i64::from(y), seed).sqrt()
                        * f64::from(radius.min(100));
                    let angle = random(i64::from(x), i64::from(y), seed.wrapping_add(1))
                        * std::f64::consts::TAU;
                    (px + distance * angle.cos(), py + distance * angle.sin())
                }
                Warp::Crystal { size, seed } => {
                    let size = f64::from(size.clamp(2, 200));
                    let (gx, gy) = ((px / size).floor() as i64, (py / size).floor() as i64);
                    let mut best = (f64::INFINITY, px, py);
                    for iy in gy - 1..=gy + 1 {
                        for ix in gx - 1..=gx + 1 {
                            let sx = (ix as f64 + 0.1 + 0.8 * random(ix, iy, seed)) * size;
                            let sy = (iy as f64 + 0.1 + 0.8 * random(ix, iy, seed.wrapping_add(1)))
                                * size;
                            let distance = (px - sx).powi(2) + (py - sy).powi(2);
                            if distance < best.0 {
                                best = (distance, sx, sy);
                            }
                        }
                    }
                    (best.1, best.2)
                }
            };
            output[y as usize * width as usize + x as usize] =
                sample(source, width, height, sx, sy);
        }
    }
    Some(output)
}
