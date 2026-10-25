use super::{AtomicBool, Effect, Ordering, Pixel};

// Five-marker piecewise-parabolic streaming quantile estimate.
struct Quantile {
    percentile: f64,
    values: [f64; 5],
    ranks: [i32; 5],
    count: usize,
}
impl Quantile {
    fn new(percentile: u8) -> Self {
        Self {
            percentile: f64::from(percentile) / 100.0,
            values: [0.0; 5],
            ranks: [0, 1, 2, 3, 4],
            count: 0,
        }
    }
    fn push(&mut self, value: u8) {
        let value = f64::from(value);
        if self.count < 5 {
            self.values[self.count] = value;
            self.count += 1;
            self.values[..self.count].sort_by(f64::total_cmp);
            return;
        }
        let bucket = self
            .values
            .iter()
            .position(|&marker| value < marker)
            .unwrap_or(5)
            .saturating_sub(1)
            .min(3);
        self.values[0] = self.values[0].min(value);
        self.values[4] = self.values[4].max(value);
        self.count += 1;
        for rank in &mut self.ranks[bucket + 1..] {
            *rank += 1;
        }
        let targets = [
            0.0,
            self.percentile / 2.0,
            self.percentile,
            (1.0 + self.percentile) / 2.0,
            1.0,
        ];
        for (index, &target) in targets.iter().enumerate().take(4).skip(1) {
            let wanted = (self.count - 1) as f64 * target - f64::from(self.ranks[index]);
            let direction = if wanted >= 1.0 {
                1
            } else if wanted <= -1.0 {
                -1
            } else {
                continue;
            };
            let neighbor = (index as i32 + direction) as usize;
            if (self.ranks[neighbor] - self.ranks[index]).abs() <= 1 {
                continue;
            }
            let left = f64::from(self.ranks[index] - self.ranks[index - 1]);
            let right = f64::from(self.ranks[index + 1] - self.ranks[index]);
            let delta = f64::from(direction);
            let lower_slope = (self.values[index] - self.values[index - 1]) / left;
            let upper_slope = (self.values[index + 1] - self.values[index]) / right;
            let prediction = self.values[index]
                + delta * ((left + delta) * upper_slope + (right - delta) * lower_slope)
                    / (left + right);
            self.values[index] =
                if self.values[index - 1] < prediction && prediction < self.values[index + 1] {
                    prediction
                } else {
                    self.values[index]
                        + delta * (self.values[neighbor] - self.values[index])
                            / f64::from(self.ranks[neighbor] - self.ranks[index])
                };
            self.ranks[index] += direction;
        }
    }
    fn value(&self) -> u8 {
        let value = if self.count < 5 {
            self.values[((self.count.saturating_sub(1)) as f64 * self.percentile).round() as usize]
        } else if self.percentile == 0.0 {
            self.values[0]
        } else if self.percentile == 1.0 {
            self.values[4]
        } else {
            self.values[2]
        };
        value.round().clamp(0.0, 255.0) as u8
    }
}
fn hash(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846ca68b);
    value ^ (value >> 16)
}
pub(super) fn render(
    source: &[Pixel],
    width: u32,
    height: u32,
    effect: Effect,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    let Effect::SketchBlur {
        radius,
        percentile,
        smoothness,
        seed,
    } = effect
    else {
        return None;
    };
    if radius > 100 || percentile > 100 || !(1..=5).contains(&smoothness) {
        return None;
    }
    if radius == 0 {
        return Some(source.to_vec());
    }
    let samples = 8usize << smoothness;
    let mut offsets = Vec::with_capacity(samples);
    for sample in 0..samples {
        let angle = sample as f64 * std::f64::consts::PI * (3.0 - 5.0f64.sqrt());
        let distance = ((sample as f64 + 0.5) / samples as f64).sqrt() * f64::from(radius);
        offsets.push((distance * angle.cos(), distance * angle.sin()));
    }
    let mut output = source.to_vec();
    for y in 0..height {
        for x in 0..width {
            if x.is_multiple_of(16) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let index = y as usize * width as usize + x as usize;
            if source[index][3] == 0 {
                continue;
            }
            let angle = f64::from(hash((index as u32) ^ seed)) / f64::from(u32::MAX)
                * std::f64::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            let mut estimators: [Quantile; 3] = std::array::from_fn(|_| Quantile::new(percentile));
            for c in 0..3 {
                estimators[c].push(source[index][c]);
            }
            for &(dx, dy) in &offsets {
                let sx = (f64::from(x) + dx * cos - dy * sin)
                    .round()
                    .clamp(0.0, f64::from(width - 1)) as usize;
                let sy = (f64::from(y) + dx * sin + dy * cos)
                    .round()
                    .clamp(0.0, f64::from(height - 1)) as usize;
                let pixel = source[sy * width as usize + sx];
                if pixel[3] > 0 {
                    for c in 0..3 {
                        estimators[c].push(pixel[c]);
                    }
                }
            }
            for (c, estimator) in estimators.iter().enumerate() {
                output[index][c] = estimator.value();
            }
        }
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streaming_quantiles_track_uniform_distribution_and_exact_endpoints() {
        for percentile in [0, 5, 25, 50, 75, 95, 100] {
            let mut estimator = Quantile::new(percentile);
            for i in 0..8192u32 {
                estimator.push((hash(i) % 256) as u8);
            }
            let expected = f64::from(percentile) * 2.55;
            assert!(
                (f64::from(estimator.value()) - expected).abs() <= 4.0,
                "percentile {percentile}: {}",
                estimator.value()
            );
            assert!(estimator.values.windows(2).all(|pair| pair[0] <= pair[1]));
            assert!(estimator.ranks.windows(2).all(|pair| pair[0] < pair[1]));
        }
    }
    #[test]
    fn constant_and_short_streams_remain_bounded() {
        for percentile in [0, 50, 100] {
            let mut estimator = Quantile::new(percentile);
            for _ in 0..100 {
                estimator.push(83);
                assert_eq!(estimator.value(), 83);
            }
        }
        let mut estimator = Quantile::new(50);
        for value in [0, 255, 128] {
            estimator.push(value);
        }
        assert_eq!(estimator.value(), 128);
    }
}
