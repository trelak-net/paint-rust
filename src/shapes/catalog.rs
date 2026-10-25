use super::*;

fn vertices(shape: Shape) -> Vec<(f64, f64)> {
    let regular = |count: usize, star: bool| {
        (0..count)
            .map(|i| {
                let angle =
                    i as f64 * std::f64::consts::TAU / count as f64 - std::f64::consts::FRAC_PI_2;
                let radius = if star && i % 2 == 1 {
                    match count {
                        6 => 0.10,
                        8 => 0.12,
                        12 => 0.29,
                        _ => 0.22,
                    }
                } else {
                    0.5
                };
                (0.5 + radius * angle.cos(), 0.5 + radius * angle.sin())
            })
            .collect()
    };
    match shape {
        Shape::Triangle => vec![(0.5, 0.0), (1.0, 1.0), (0.0, 1.0)],
        Shape::RightTriangle => vec![(0.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
        Shape::Diamond => vec![(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5)],
        Shape::Pentagon => regular(5, false),
        Shape::Hexagon => regular(6, false),
        Shape::Heptagon => regular(7, false),
        Shape::Octagon => regular(8, false),
        Shape::StarThree => regular(6, true),
        Shape::StarFour => regular(8, true),
        Shape::StarSix => regular(12, true),
        Shape::Star => regular(10, true),
        Shape::Trapezoid => vec![(0.25, 0.0), (0.75, 0.0), (1.0, 1.0), (0.0, 1.0)],
        Shape::Parallelogram => vec![(0.25, 0.0), (1.0, 0.0), (0.75, 1.0), (0.0, 1.0)],
        Shape::ArrowRight | Shape::ArrowLeft | Shape::ArrowUp | Shape::ArrowDown => {
            let points = [
                (0.0, 0.3),
                (0.6, 0.3),
                (0.6, 0.0),
                (1.0, 0.5),
                (0.6, 1.0),
                (0.6, 0.7),
                (0.0, 0.7),
            ];
            points
                .into_iter()
                .map(|(x, y)| match shape {
                    Shape::ArrowLeft => (1.0 - x, y),
                    Shape::ArrowUp => (y, 1.0 - x),
                    Shape::ArrowDown => (y, x),
                    _ => (x, y),
                })
                .collect()
        }
        Shape::ArrowNotched => vec![
            (0.0, 0.25),
            (0.6, 0.25),
            (0.6, 0.0),
            (1.0, 0.5),
            (0.6, 1.0),
            (0.6, 0.75),
            (0.0, 0.75),
            (0.2, 0.5),
        ],
        Shape::ArrowPentagon => vec![(0.0, 0.0), (0.55, 0.0), (1.0, 0.5), (0.55, 1.0), (0.0, 1.0)],
        Shape::ArrowChevron => vec![
            (0.0, 0.0),
            (0.6, 0.0),
            (1.0, 0.5),
            (0.6, 1.0),
            (0.0, 1.0),
            (0.4, 0.5),
        ],
        Shape::CalloutRectangle => vec![
            (0.0, 0.0),
            (1.0, 0.0),
            (1.0, 0.8),
            (0.35, 0.8),
            (0.08, 1.0),
            (0.15, 0.8),
            (0.0, 0.8),
        ],
        Shape::CalloutRounded => {
            let mut points = Vec::with_capacity(39);
            for (corner, (cx, cy)) in [(0.9, 0.1), (0.9, 0.7), (0.1, 0.7), (0.1, 0.1)]
                .into_iter()
                .enumerate()
            {
                if corner == 2 {
                    points.extend([(0.35, 0.8), (0.08, 1.0), (0.15, 0.8)]);
                }
                for step in 0..=8 {
                    let angle =
                        (corner as f64 - 1.0 + step as f64 / 8.0) * std::f64::consts::FRAC_PI_2;
                    points.push((cx + 0.1 * angle.cos(), cy + 0.1 * angle.sin()));
                }
            }
            points
        }
        Shape::CalloutEllipse => {
            let mut points = Vec::with_capacity(97);
            for i in 0..96 {
                if i == 30 {
                    points.push((0.08, 1.0));
                }
                if (30..36).contains(&i) {
                    continue;
                }
                let angle = i as f64 * std::f64::consts::TAU / 96.0;
                points.push((0.5 + 0.5 * angle.cos(), 0.4 + 0.4 * angle.sin()));
            }
            points
        }
        Shape::CalloutCloud => (0..120)
            .map(|i| {
                let angle = i as f64 * std::f64::consts::TAU / 120.0;
                let radius = 1.0 + 0.08 * (10.0 * angle).cos();
                (
                    0.56 + 0.4 * radius * angle.cos(),
                    0.36 + 0.32 * radius * angle.sin(),
                )
            })
            .collect(),
        Shape::Lightning => vec![
            (0.25, 0.0),
            (0.7, 0.0),
            (0.5, 0.38),
            (0.82, 0.38),
            (0.6, 0.6),
            (1.0, 1.0),
            (0.18, 0.48),
            (0.43, 0.48),
            (0.0, 0.18),
        ],
        Shape::CheckMark => vec![
            (0.0, 0.5),
            (0.16, 0.34),
            (0.4, 0.61),
            (0.85, 0.0),
            (1.0, 0.14),
            (0.42, 1.0),
        ],
        Shape::Multiply => vec![
            (0.15, 0.0),
            (0.5, 0.35),
            (0.85, 0.0),
            (1.0, 0.15),
            (0.65, 0.5),
            (1.0, 0.85),
            (0.85, 1.0),
            (0.5, 0.65),
            (0.15, 1.0),
            (0.0, 0.85),
            (0.35, 0.5),
            (0.0, 0.15),
        ],
        Shape::Gear => (0..32)
            .map(|i| {
                let angle = (i as f64 + 0.5) * std::f64::consts::TAU / 32.0;
                let radius = if matches!(i % 4, 1 | 2) { 0.5 } else { 0.4 };
                (0.5 + radius * angle.cos(), 0.5 + radius * angle.sin())
            })
            .collect(),
        Shape::Heart => (0..96)
            .map(|i| {
                let t = i as f64 * std::f64::consts::TAU / 96.0;
                let x = 16.0 * t.sin().powi(3);
                let y = 13.0 * t.cos()
                    - 5.0 * (2.0 * t).cos()
                    - 2.0 * (3.0 * t).cos()
                    - (4.0 * t).cos();
                ((x + 16.0) / 32.0, (12.0 - y) / 29.0)
            })
            .collect(),
        _ => vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
    }
}
fn polygon_distance(point: (f64, f64), points: &[(f64, f64)]) -> f64 {
    let (x, y) = point;
    let mut inside = false;
    let mut distance = f64::INFINITY;
    for i in 0..points.len() {
        let (ax, ay) = points[i];
        let (bx, by) = points[(i + 1) % points.len()];
        if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
            inside = !inside;
        }
        let (dx, dy) = (bx - ax, by - ay);
        let length = dx * dx + dy * dy;
        let t = if length > 0.0 {
            ((x - ax) * dx + (y - ay) * dy) / length
        } else {
            0.0
        }
        .clamp(0.0, 1.0);
        distance = distance.min((x - ax - t * dx).hypot(y - ay - t * dy));
    }
    if inside { distance } else { -distance }
}

pub(super) fn render(
    source: &[Pixel],
    dimensions: (u32, u32),
    selection: Option<&Selection>,
    parameters: Parameters,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    let Parameters {
        shape,
        from,
        to,
        style,
    } = parameters;
    let (width, height) = dimensions;
    if shape == Shape::Line || (from.0 - to.0).abs() < 1.0 || (from.1 - to.1).abs() < 1.0 {
        let mut pixels = source.to_vec();
        // The shared brush kernel supplies continuous antialiased line coverage.
        let mut coverage = vec![0; source.len()];
        crate::brush::render(
            &mut pixels,
            source,
            &mut coverage,
            dimensions,
            selection,
            crate::brush::Segment {
                clone_source: None,
                from,
                to,
                diameter: style.width,
                paint: crate::brush::Paint::Color(style.outline.unwrap_or(style.color)),
                options: crate::brush::Options {
                    antialias: style.antialias,
                    blend: style.paint_mode.is_some(),
                    blend_mode: style.paint_mode.unwrap_or_default(),
                    ..Default::default()
                },
            },
            cancel,
        );
        return (!cancel.load(Ordering::Relaxed)).then_some(pixels);
    }
    let (left, right) = (f64::from(from.0.min(to.0)), f64::from(from.0.max(to.0)));
    let (top, bottom) = (f64::from(from.1.min(to.1)), f64::from(from.1.max(to.1)));
    let (rx, ry) = ((right - left) / 2.0, (bottom - top) / 2.0);
    let (cx, cy) = ((left + right) / 2.0, (top + bottom) / 2.0);
    let points: Vec<_> = vertices(shape)
        .into_iter()
        .map(|(x, y)| (left + x * (right - left), top + y * (bottom - top)))
        .collect();
    let mut output = source.to_vec();
    let coverage = |distance: f64| {
        if style.antialias {
            (distance + 0.5).clamp(0.0, 1.0)
        } else {
            f64::from(distance >= 0.0)
        }
    };
    for y in (top - 1.0).floor().max(0.0) as u32
        ..(bottom + 1.0).ceil().clamp(0.0, f64::from(height)) as u32
    {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        for x in (left - 1.0).floor().max(0.0) as u32
            ..(right + 1.0).ceil().clamp(0.0, f64::from(width)) as u32
        {
            if x.is_multiple_of(256) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            if selection.is_some_and(|s| !s.contains(x as usize, y as usize)) {
                continue;
            }
            let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let distance = match shape {
                Shape::Ellipse => {
                    let (dx, dy) = (px - cx, py - cy);
                    let q = (dx / rx).powi(2) + (dy / ry).powi(2);
                    let gradient = 2.0 * (dx / (rx * rx)).hypot(dy / (ry * ry));
                    if gradient > 0.0 {
                        (1.0 - q) / gradient
                    } else {
                        rx.min(ry)
                    }
                }
                Shape::RoundedRectangle => {
                    let radius = rx.min(ry) * 0.4;
                    let (qx, qy) = ((px - cx).abs() - rx + radius, (py - cy).abs() - ry + radius);
                    radius - qx.max(qy).min(0.0) - qx.max(0.0).hypot(qy.max(0.0))
                }
                Shape::Gear => polygon_distance((px, py), &points)
                    .min((((px - cx) / rx).hypot((py - cy) / ry) - 0.32) * rx.min(ry)),
                Shape::CalloutCloud => {
                    let bubble = |x: f64, y: f64, radius: f64| {
                        (radius
                            - ((px - left) / (right - left) - x)
                                .hypot((py - top) / (bottom - top) - y))
                            * (right - left).min(bottom - top)
                    };
                    polygon_distance((px, py), &points)
                        .max(bubble(0.24, 0.76, 0.085))
                        .max(bubble(0.1, 0.94, 0.05))
                }
                _ => polygon_distance((px, py), &points),
            };
            let outer = coverage(distance);
            if outer <= 0.0 {
                continue;
            }
            let outline = (outer - coverage(distance - f64::from(style.width.max(1.0)))).max(0.0);
            let (color, amount) = if style.filled {
                let color = if let Some(border) = style.outline {
                    crate::brush::composite(
                        style.color,
                        border,
                        (outline / outer * 255.0).round() as u8,
                        false,
                        false,
                    )
                } else {
                    style.color
                };
                (color, outer)
            } else {
                (style.color, outline)
            };
            let i = y as usize * width as usize + x as usize;
            let amount = (amount * 255.0).round() as u8;
            output[i] = if let Some(mode) = style.paint_mode {
                crate::document::blend_layer(source[i], color, amount, mode)
            } else {
                crate::brush::composite(source[i], color, amount, false, false)
            };
        }
    }
    Some(output)
}
