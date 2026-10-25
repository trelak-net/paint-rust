//! Bounded cubic Bezier strokes with adaptive subdivision and shared coverage.
use crate::{
    brush,
    document::{Pixel, Selection, ShapeStyle},
};
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameters {
    pub points: [(f32, f32); 4],
    pub style: ShapeStyle,
}
type Point = (f64, f64);
fn midpoint(a: Point, b: Point) -> Point {
    ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5)
}
fn flatten(
    points: [Point; 4],
    depth: u8,
    output: &mut Vec<(f32, f32)>,
    cancel: &AtomicBool,
) -> Option<()> {
    if cancel.load(Ordering::Relaxed) || output.len() >= 4097 {
        return None;
    }
    let [a, b, c, d] = points;
    let (dx, dy) = (d.0 - a.0, d.1 - a.1);
    let length = dx * dx + dy * dy;
    // Distance to the finite chord also catches collinear reversals and cusps.
    let distance = |p: Point| {
        let t = if length > 0.0 {
            ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length
        } else {
            0.0
        };
        let t = t.clamp(0.0, 1.0);
        (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
    };
    if distance(b).max(distance(c)) <= 0.2 {
        output.push((d.0 as f32, d.1 as f32));
        return Some(());
    }
    if depth == 16 {
        return None;
    }
    let (ab, bc, cd) = (midpoint(a, b), midpoint(b, c), midpoint(c, d));
    let (abc, bcd) = (midpoint(ab, bc), midpoint(bc, cd));
    let middle = midpoint(abc, bcd);
    flatten([a, ab, abc, middle], depth + 1, output, cancel)?;
    flatten([middle, bcd, cd, d], depth + 1, output, cancel)
}
pub fn render(
    source: &[Pixel],
    width: u32,
    height: u32,
    selection: Option<&Selection>,
    parameters: Parameters,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    if cancel.load(Ordering::Relaxed)
        || crate::document::Document::validate_size(width, height).is_err()
        || u64::from(width) * u64::from(height) != source.len() as u64
        || parameters.points.iter().any(|p| {
            !p.0.is_finite() || !p.1.is_finite() || p.0.abs() > 32768.0 || p.1.abs() > 32768.0
        })
        || !parameters.style.width.is_finite()
        || parameters.style.width <= 0.0
        || parameters.style.width > 4096.0
    {
        return None;
    }
    let mut points = vec![parameters.points[0]];
    flatten(
        parameters.points.map(|p| (f64::from(p.0), f64::from(p.1))),
        0,
        &mut points,
        cancel,
    )?;
    let mut output = source.to_vec();
    let mut coverage = vec![0; source.len()];
    let style = parameters.style;
    for pair in points.windows(2) {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        brush::render(
            &mut output,
            source,
            &mut coverage,
            (width, height),
            selection,
            brush::Segment {
                clone_source: None,
                from: pair[0],
                to: pair[1],
                diameter: style.width,
                paint: brush::Paint::Color(style.outline.unwrap_or(style.color)),
                options: brush::Options {
                    antialias: style.antialias,
                    blend: style.paint_mode.is_some(),
                    blend_mode: style.paint_mode.unwrap_or_default(),
                    ..Default::default()
                },
            },
            cancel,
        );
    }
    (!cancel.load(Ordering::Relaxed)).then_some(output)
}
