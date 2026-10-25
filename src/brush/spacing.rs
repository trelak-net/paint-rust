//! Arc-length sampling with off-canvas work skipped before rasterization.
#[derive(Clone, Default)]
pub(crate) struct Spacing {
    phase: f64,
    step: f64,
    started: bool,
}
impl Spacing {
    pub(crate) fn visit(
        &mut self,
        from: (f32, f32),
        to: (f32, f32),
        diameter: f32,
        percent: f32,
        size: (u32, u32),
        mut dab: impl FnMut((f32, f32)),
    ) {
        if ![from.0, from.1, to.0, to.1, diameter, percent]
            .into_iter()
            .all(f32::is_finite)
        {
            return;
        }
        let step = (f64::from(diameter.clamp(1.0, 4096.0)) * f64::from(percent.clamp(0.0, 1000.0))
            / 100.0)
            .max(0.25);
        if self.step != step {
            *self = Self {
                step,
                ..Self::default()
            };
        }
        let (x, y) = (f64::from(from.0), f64::from(from.1));
        let (dx, dy) = (f64::from(to.0) - x, f64::from(to.1) - y);
        let length = dx.hypot(dy);
        let first = if self.started { step - self.phase } else { 0.0 };
        self.started = true;
        self.phase = (self.phase + length) % step;
        let radius = f64::from(diameter.clamp(1.0, 4096.0)) / 2.0 + 1.0;
        let (mut low, mut high) = (0.0_f64, 1.0_f64);
        for (origin, delta, upper) in [(x, dx, f64::from(size.0)), (y, dy, f64::from(size.1))] {
            if delta == 0.0 {
                if origin < -radius || origin > upper + radius {
                    return;
                }
            } else {
                let a = (-radius - origin) / delta;
                let b = (upper + radius - origin) / delta;
                low = low.max(a.min(b));
                high = high.min(a.max(b));
            }
        }
        if low > high {
            return;
        }
        if length == 0.0 {
            if first == 0.0 {
                dab(from);
            }
            return;
        }
        let skip = ((low * length - first) / step).ceil().max(0.0);
        let first_visible = first + skip * step;
        let count = ((high * length - first_visible) / step).floor() as i64 + 1;
        for index in 0..count {
            let t = (first_visible + index as f64 * step) / length;
            dab(((x + t * dx) as f32, (y + t * dy) as f32));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invisible_path_is_skipped_without_changing_visible_phase() {
        let mut spacing = Spacing::default();
        let mut points = Vec::new();
        spacing.visit(
            (-1_000_000.0, 5.0),
            (1_000_000.0, 5.0),
            2.0,
            100.0,
            (20, 10),
            |p| points.push(p),
        );
        assert_eq!(points.len(), 13);
        assert_eq!(points.first(), Some(&(-2.0, 5.0)));
        assert_eq!(points.last(), Some(&(22.0, 5.0)));
    }
    #[test]
    fn bends_keep_arc_length_and_stationary_events_do_not_duplicate_dabs() {
        let mut spacing = Spacing::default();
        let mut points = Vec::new();
        for (a, b) in [
            ((0.0, 0.0), (3.0, 0.0)),
            ((3.0, 0.0), (3.0, 0.0)),
            ((3.0, 0.0), (3.0, 5.0)),
        ] {
            spacing.visit(a, b, 2.0, 100.0, (10, 10), |p| points.push(p));
        }
        assert_eq!(
            points,
            [(0.0, 0.0), (2.0, 0.0), (3.0, 1.0), (3.0, 3.0), (3.0, 5.0)]
        );
    }
}
