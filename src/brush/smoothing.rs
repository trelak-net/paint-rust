//! Distance-domain exponential stabilization with bounded curve flattening.
type Point = (f64, f64);
#[derive(Clone, Default)]
pub(crate) struct Smoothing {
    input: Option<(f32, f32)>,
    output: Point,
}
impl Smoothing {
    pub(crate) fn continues(&self, from: (f32, f32)) -> bool {
        self.input.is_none_or(|input| input == from)
    }

    pub(crate) fn visit(
        &mut self,
        from: (f32, f32),
        to: (f32, f32),
        radius: f32,
    ) -> Option<Vec<(f32, f32)>> {
        if ![from.0, from.1, to.0, to.1, radius]
            .into_iter()
            .all(f32::is_finite)
            || radius <= 0.0
        {
            return None;
        }
        let start = (f64::from(from.0), f64::from(from.1));
        let end = (f64::from(to.0), f64::from(to.1));
        if self.input.is_none() || !self.continues(from) {
            self.output = start;
        }
        self.input = Some(to);
        let initial = self.output;
        let delta = (end.0 - start.0, end.1 - start.1);
        let length = delta.0.hypot(delta.1);
        let mut points = vec![(initial.0 as f32, initial.1 as f32)];
        if length == 0.0 {
            points.push(points[0]);
            return Some(points);
        }
        let unit = (delta.0 / length, delta.1 / length);
        let radius = f64::from(radius.clamp(0.1, 256.0));
        let at = |distance: f64| {
            let decay = (-distance / radius).exp();
            (
                start.0 + unit.0 * distance + (initial.0 - start.0) * decay
                    - unit.0 * radius * (1.0 - decay),
                start.1 + unit.1 * distance + (initial.1 - start.1) * decay
                    - unit.1 * radius * (1.0 - decay),
            )
        };
        // After sixteen response lengths the remaining curvature is subpixel.
        // Split at coordinate extrema first so reversals cannot hide in a chord.
        let transient = length.min(radius * 16.0);
        let mut stops = vec![0.0, transient];
        for (velocity, residual) in [
            (unit.0, (start.0 - initial.0) / radius - unit.0),
            (unit.1, (start.1 - initial.1) / radius - unit.1),
        ] {
            let decay = -velocity / residual;
            if decay > 0.0 && decay < 1.0 {
                let distance = -radius * decay.ln();
                if distance < transient {
                    stops.push(distance);
                }
            }
        }
        stops.sort_by(f64::total_cmp);
        for span in stops.windows(2) {
            flatten(&at, span[0], span[1], 0, &mut points);
        }
        if length > transient {
            let p = at(length);
            points.push((p.0 as f32, p.1 as f32));
        }
        self.output = at(length);
        Some(points)
    }
}

fn flatten(
    at: &impl Fn(f64) -> Point,
    low: f64,
    high: f64,
    depth: u8,
    output: &mut Vec<(f32, f32)>,
) {
    let (a, b, middle) = (at(low), at(high), at((low + high) * 0.5));
    let chord = (b.0 - a.0, b.1 - a.1);
    let length = chord.0.hypot(chord.1);
    let error = if length > 0.0 {
        ((middle.0 - a.0) * chord.1 - (middle.1 - a.1) * chord.0).abs() / length
    } else {
        (middle.0 - a.0).hypot(middle.1 - a.1)
    };
    if error > 0.125 && depth < 7 {
        let mid = (low + high) * 0.5;
        flatten(at, low, mid, depth + 1, output);
        flatten(at, mid, high, depth + 1, output);
    } else {
        output.push((b.0 as f32, b.1 as f32));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subdivision_preserves_filter_state_and_straight_paths_stay_straight() {
        let mut whole = Smoothing::default();
        let mut split = Smoothing::default();
        whole.visit((0.0, 0.0), (100.0, 0.0), 12.0).unwrap();
        for x in 0..100 {
            split
                .visit((x as f32, 0.0), ((x + 1) as f32, 0.0), 12.0)
                .unwrap();
        }
        assert!((whole.output.0 - split.output.0).abs() < 1e-9);
        assert_eq!(whole.output.1, 0.0);
        assert!((whole.output.0 - 88.0).abs() < 0.01);
    }
    #[test]
    fn reversals_retain_the_turning_point_and_huge_paths_have_bounded_work() {
        let mut filter = Smoothing::default();
        filter.visit((0.0, 0.0), (100.0, 0.0), 12.0).unwrap();
        let start = filter.output.0 as f32;
        let reversal = filter.visit((100.0, 0.0), (0.0, 0.0), 12.0).unwrap();
        assert!(reversal.iter().any(|point| point.0 > start + 1.0));
        let huge = filter.visit((0.0, 0.0), (1e20, 1e20), 256.0).unwrap();
        assert!(huge.len() <= 386);
        assert!(
            huge.iter()
                .all(|point| point.0.is_finite() && point.1.is_finite())
        );
        assert!(filter.visit((f32::NAN, 0.0), (0.0, 0.0), 12.0).is_none());
    }
}
