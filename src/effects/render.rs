use super::{AtomicBool, Effect, Ordering, Pixel};

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NoiseSettings {
    pub scale: f32,
    pub roughness: f32,
    pub seed: u32,
    pub colors: [Pixel; 2],
}
impl Default for NoiseSettings {
    fn default() -> Self {
        Self {
            scale: 100.0,
            roughness: 0.5,
            seed: 1,
            colors: [[0, 0, 0, 255], [255; 4]],
        }
    }
}
impl NoiseSettings {
    pub fn valid(self) -> bool {
        self.scale.is_finite()
            && (1.0..=1000.0).contains(&self.scale)
            && self.roughness.is_finite()
            && (0.0..=1.0).contains(&self.roughness)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FractalSettings {
    pub iterations: u32,
    pub zoom: f32,
    pub center: [f32; 2],
    pub angle: f32,
    pub julia: [f32; 2],
    pub colors: [Pixel; 2],
}
impl Default for FractalSettings {
    fn default() -> Self {
        Self {
            iterations: 100,
            zoom: 1.0,
            center: [-0.5, 0.0],
            angle: 0.0,
            julia: [-0.7, 0.27015],
            colors: [[0, 0, 0, 255], [255; 4]],
        }
    }
}
impl FractalSettings {
    pub fn valid(self) -> bool {
        (1..=512).contains(&self.iterations)
            && self.zoom.is_finite()
            && (0.1..=1000.0).contains(&self.zoom)
            && self.angle.is_finite()
            && (-180.0..=180.0).contains(&self.angle)
            && self
                .center
                .into_iter()
                .all(|v| v.is_finite() && (-4.0..=4.0).contains(&v))
            && self
                .julia
                .into_iter()
                .all(|v| v.is_finite() && (-2.0..=2.0).contains(&v))
    }
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
fn noise(x: f64, y: f64, seed: u32) -> f64 {
    let (ix, iy) = (x.floor() as i64, y.floor() as i64);
    let smooth = |t: f64| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (tx, ty) = (smooth(x - x.floor()), smooth(y - y.floor()));
    let mix = |a: f64, b: f64, t: f64| a + (b - a) * t;
    mix(
        mix(random(ix, iy, seed), random(ix + 1, iy, seed), tx),
        mix(random(ix, iy + 1, seed), random(ix + 1, iy + 1, seed), tx),
        ty,
    )
}
fn color(colors: [Pixel; 2], amount: f64) -> Pixel {
    if amount <= 0.0 {
        return colors[0];
    }
    if amount >= 1.0 {
        return colors[1];
    }
    let a = f64::from(colors[0][3]) * (1.0 - amount);
    let b = f64::from(colors[1][3]) * amount;
    if a + b == 0.0 {
        return [0; 4];
    }
    let mut output = [0; 4];
    for (c, channel) in output[..3].iter_mut().enumerate() {
        *channel =
            ((f64::from(colors[0][c]) * a + f64::from(colors[1][c]) * b) / (a + b)).round() as u8;
    }
    output[3] = (a + b).round() as u8;
    output
}

pub(super) fn render(
    width: u32,
    height: u32,
    effect: Effect,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    if !effect.valid_settings() {
        return None;
    }
    let mut output = vec![[0; 4]; width as usize * height as usize];
    for y in 0..height {
        for x in 0..width {
            if x.is_multiple_of(64) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let pixel = match effect {
                Effect::Clouds(settings) | Effect::Turbulence(settings) => {
                    let turbulent = matches!(effect, Effect::Turbulence(_));
                    let (mut frequency, mut weight, mut sum, mut weights) = (1.0, 1.0, 0.0, 0.0);
                    for octave in 0..8 {
                        let value = noise(
                            f64::from(x) / f64::from(settings.scale) * frequency,
                            f64::from(y) / f64::from(settings.scale) * frequency,
                            settings.seed.wrapping_add(octave),
                        );
                        sum += weight
                            * if turbulent {
                                (value * 2.0 - 1.0).abs()
                            } else {
                                value
                            };
                        weights += weight;
                        frequency *= 2.0;
                        weight *= f64::from(settings.roughness);
                    }
                    color(settings.colors, sum / weights)
                }
                Effect::Mandelbrot(settings) | Effect::Julia(settings) => {
                    // Pixel centers map to a square complex-plane scale without stretching.
                    let scale = 4.0 / (f64::from(width.min(height)) * f64::from(settings.zoom));
                    let (dx, dy) = (
                        (f64::from(x) + 0.5 - f64::from(width) / 2.0) * scale,
                        (f64::from(y) + 0.5 - f64::from(height) / 2.0) * scale,
                    );
                    let angle = f64::from(settings.angle).to_radians();
                    let point = [
                        f64::from(settings.center[0]) + dx * angle.cos() - dy * angle.sin(),
                        f64::from(settings.center[1]) + dx * angle.sin() + dy * angle.cos(),
                    ];
                    let (mut z, c) = if matches!(effect, Effect::Julia(_)) {
                        (point, settings.julia.map(f64::from))
                    } else {
                        ([0.0; 2], point)
                    };
                    let mut escaped = None;
                    for step in 0..settings.iterations {
                        if step.is_multiple_of(64) && cancel.load(Ordering::Relaxed) {
                            return None;
                        }
                        z = [z[0] * z[0] - z[1] * z[1] + c[0], 2.0 * z[0] * z[1] + c[1]];
                        if z[0] * z[0] + z[1] * z[1] > 4.0 {
                            escaped = Some(step);
                            break;
                        }
                    }
                    let amount = escaped.map_or(0.0, |step| {
                        1.0 - f64::from(step) / f64::from(settings.iterations)
                    });
                    color(settings.colors, amount)
                }
                _ => return None,
            };
            output[y as usize * width as usize + x as usize] = pixel;
        }
    }
    Some(output)
}
