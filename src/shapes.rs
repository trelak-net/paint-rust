mod catalog;
use crate::document::{Pixel, Selection, Shape, ShapeStyle};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameters {
    pub shape: Shape,
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub style: ShapeStyle,
}

pub(crate) struct Stroke {
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub diameter: f32,
    pub color: Pixel,
    pub erase: bool,
}

pub(crate) fn stroke(
    pixels: &mut [Pixel],
    dimensions: (u32, u32),
    selection: Option<&Selection>,
    parameters: Stroke,
    cancel: &AtomicBool,
) -> bool {
    let Stroke {
        from,
        to,
        diameter,
        color,
        erase,
    } = parameters;
    let (width, height) = dimensions;
    let radius = diameter.max(1.0) / 2.0;
    let min_x = ((from.0.min(to.0) - radius).floor() as i32).max(0);
    let max_x = ((from.0.max(to.0) + radius).ceil() as i32).min(width as i32 - 1);
    let min_y = ((from.1.min(to.1) - radius).floor() as i32).max(0);
    let max_y = ((from.1.max(to.1) + radius).ceil() as i32).min(height as i32 - 1);
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let length_sq = dx * dx + dy * dy;
    // Bound candidate pixels by the segment's horizontal span in this row's
    // radius band. A numeric margin keeps the original f32 coverage test decisive.
    let margin = 1.0
        + 8.0
            * f64::from(f32::EPSILON)
            * [from.0, from.1, to.0, to.1]
                .iter()
                .map(|v| f64::from(v.abs()))
                .sum::<f64>();
    let band = f64::from(radius) + margin;
    let vertical = f64::from(to.1) - f64::from(from.1);
    let horizontal = f64::from(to.0) - f64::from(from.0);
    for y in min_y..=max_y {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let (row_min, row_max) = if vertical.abs() > f64::from(f32::EPSILON) {
            let py = f64::from(y) + 0.5 - f64::from(from.1);
            let a = ((py - band) / vertical).clamp(0.0, 1.0);
            let b = ((py + band) / vertical).clamp(0.0, 1.0);
            let ax = f64::from(from.0) + a * horizontal;
            let bx = f64::from(from.0) + b * horizontal;
            (
                min_x.max((ax.min(bx) - band).floor() as i32),
                max_x.min((ax.max(bx) + band).ceil() as i32),
            )
        } else {
            (min_x, max_x)
        };
        for x in row_min..=row_max {
            if (x as u32).is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
                return false;
            }
            if !selection.is_none_or(|s| s.contains(x as usize, y as usize)) {
                continue;
            }
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let t = if length_sq > 0.0 {
                ((px - from.0) * dx + (py - from.1) * dy) / length_sq
            } else {
                0.0
            }
            .clamp(0.0, 1.0);
            if (px - from.0 - t * dx).powi(2) + (py - from.1 - t * dy).powi(2) <= radius * radius {
                let i = y as usize * width as usize + x as usize;
                pixels[i] = if erase { [0; 4] } else { color };
            }
        }
    }
    !cancel.load(Ordering::Relaxed)
}

pub fn render(
    source: &[Pixel],
    width: u32,
    height: u32,
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
    if cancel.load(Ordering::Relaxed)
        || width == 0
        || height == 0
        || u64::from(width) * u64::from(height) != source.len() as u64
        || [from.0, from.1, to.0, to.1, style.width]
            .iter()
            .any(|v| !v.is_finite())
    {
        return None;
    }
    if style.paint_mode.is_some()
        || style.antialias
        || style.outline.is_some()
        || !matches!(shape, Shape::Line | Shape::Rectangle | Shape::Ellipse)
    {
        return catalog::render(source, (width, height), selection, parameters, cancel);
    }
    let mut pixels = source.to_vec();
    if shape == Shape::Line || (from.0 - to.0).abs() < 1.0 || (from.1 - to.1).abs() < 1.0 {
        return stroke(
            &mut pixels,
            (width, height),
            selection,
            Stroke {
                from,
                to,
                diameter: style.width,
                color: style.color,
                erase: false,
            },
            cancel,
        )
        .then_some(pixels);
    }
    let (left, right) = (from.0.min(to.0), from.0.max(to.0));
    let (top, bottom) = (from.1.min(to.1), from.1.max(to.1));
    let (cx, cy) = ((left + right) * 0.5, (top + bottom) * 0.5);
    let (rx, ry) = ((right - left) * 0.5, (bottom - top) * 0.5);
    let thickness = style.width.max(1.0);
    for y in (top.floor() as i32).max(0)..(bottom.ceil() as i32).min(height as i32) {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        for x in (left.floor() as i32).max(0)..(right.ceil() as i32).min(width as i32) {
            if (x as u32).is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let painted = match shape {
                Shape::Rectangle => {
                    px >= left
                        && px <= right
                        && py >= top
                        && py <= bottom
                        && (style.filled
                            || px < left + thickness
                            || px > right - thickness
                            || py < top + thickness
                            || py > bottom - thickness)
                }
                Shape::Ellipse => {
                    ((px - cx) / rx).powi(2) + ((py - cy) / ry).powi(2) <= 1.0
                        && (style.filled
                            || rx <= thickness
                            || ry <= thickness
                            || ((px - cx) / (rx - thickness)).powi(2)
                                + ((py - cy) / (ry - thickness)).powi(2)
                                >= 1.0)
                }
                _ => unreachable!(),
            };
            if painted && selection.is_none_or(|s| s.contains(x as usize, y as usize)) {
                pixels[y as usize * width as usize + x as usize] = style.color;
            }
        }
    }
    if cancel.load(Ordering::Relaxed) {
        None
    } else {
        Some(pixels)
    }
}
