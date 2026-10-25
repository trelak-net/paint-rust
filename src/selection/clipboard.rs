use super::*;
use std::collections::HashMap;
const MAX_TEXT: usize = 8 * 1024 * 1024;
const MAX_POLYGONS: usize = 16384;
const MAX_VERTICES: usize = 81920;
#[derive(serde::Serialize, serde::Deserialize)]
struct Geometry {
    #[serde(rename = "polygonList")]
    polygons: Vec<String>,
}
/// Encode a binary selection as vertically merged, non-overlapping rectangle polygons.
pub fn encode(
    width: u32,
    height: u32,
    selection: Option<&Selection>,
    cancel: &AtomicBool,
) -> Result<String, String> {
    check_cancel(cancel)?;
    crate::document::Document::validate_size(width, height)?;
    let mut rectangles: Vec<[u32; 4]> = Vec::new();
    let mut previous: HashMap<(u32, u32), usize> = HashMap::new();
    for y in 0..height {
        check_cancel(cancel)?;
        let mut current = HashMap::new();
        let mut x = 0;
        while x < width {
            if x.is_multiple_of(4096) {
                check_cancel(cancel)?;
            }
            if !selection.is_none_or(|s| s.contains(x as usize, y as usize)) {
                x += 1;
                continue;
            }
            let start = x;
            x += 1;
            while x < width && selection.is_none_or(|s| s.contains(x as usize, y as usize)) {
                if x.is_multiple_of(4096) {
                    check_cancel(cancel)?;
                }
                x += 1;
            }
            let span = (start, x);
            let index = if let Some(&index) = previous.get(&span) {
                rectangles[index][3] = y + 1;
                index
            } else {
                if rectangles.len() >= MAX_POLYGONS {
                    return Err("Selection geometry is too complex for the clipboard".into());
                }
                rectangles.push([start, y, x, y + 1]);
                rectangles.len() - 1
            };
            current.insert(span, index);
        }
        previous = current;
    }
    let mut polygons = Vec::with_capacity(rectangles.len());
    for (index, [left, top, right, bottom]) in rectangles.into_iter().enumerate() {
        if index.is_multiple_of(256) {
            check_cancel(cancel)?;
        }
        polygons.push(format!(
            "{left},{top},{right},{top},{right},{bottom},{left},{bottom},{left},{top}"
        ));
    }
    check_cancel(cancel)?;
    let text = serde_json::to_string(&Geometry { polygons }).map_err(|error| error.to_string())?;
    if text.len() > MAX_TEXT {
        return Err("Selection clipboard text exceeds 8 MiB".into());
    }
    Ok(text)
}
struct Edge {
    start: u32,
    end: u32,
    x: f64,
    y: f64,
    slope: f64,
}
/// Rasterize polygon lists with even-odd filling, preserving holes and clipping to canvas.
pub fn decode(
    width: u32,
    height: u32,
    text: &str,
    cancel: &AtomicBool,
) -> Result<Selection, String> {
    check_cancel(cancel)?;
    crate::document::Document::validate_size(width, height)?;
    if text.len() > MAX_TEXT {
        return Err("Selection clipboard text exceeds 8 MiB".into());
    }
    let geometry: Geometry = serde_json::from_str(text)
        .map_err(|error| format!("Invalid selection geometry: {error}"))?;
    if geometry.polygons.len() > MAX_POLYGONS {
        return Err("Too many selection polygons".into());
    }
    let mut edges = Vec::new();
    let mut count = 0usize;
    for polygon in geometry.polygons {
        check_cancel(cancel)?;
        let mut values = Vec::new();
        for value in polygon.split(',') {
            if values.len().is_multiple_of(256) {
                check_cancel(cancel)?;
            }
            if values.len() > MAX_VERTICES * 2 {
                return Err("Too many selection vertices".into());
            }
            let value = value
                .trim()
                .parse::<f64>()
                .map_err(|_| "Invalid selection coordinate")?;
            if !value.is_finite() || value.abs() > 1e9 {
                return Err("Selection coordinate is out of range".into());
            }
            values.push(value);
        }
        if values.len() < 6 || !values.len().is_multiple_of(2) {
            return Err("A selection polygon needs at least three coordinate pairs".into());
        }
        count += values.len() / 2;
        if count > MAX_VERTICES {
            return Err("Too many selection vertices".into());
        }
        let points = values.as_chunks::<2>().0;
        for (i, &[ax, ay]) in points.iter().enumerate() {
            if i.is_multiple_of(256) {
                check_cancel(cancel)?;
            }
            let [bx, by] = points[(i + 1) % points.len()];
            if ay == by {
                continue;
            }
            let start = (ay.min(by) - 0.5).ceil().clamp(0.0, f64::from(height)) as u32;
            let end = (ay.max(by) - 0.5).ceil().clamp(0.0, f64::from(height)) as u32;
            if start < end {
                edges.push(Edge {
                    start,
                    end,
                    x: ax,
                    y: ay,
                    slope: (bx - ax) / (by - ay),
                });
            }
        }
    }
    edges.sort_unstable_by_key(|edge| edge.start);
    let mut active: Vec<usize> = Vec::new();
    let mut next = 0;
    let mut intersections = Vec::new();
    let mut pixels = vec![0; width as usize * height as usize];
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for y in 0..height {
        check_cancel(cancel)?;
        active.retain(|&index| edges[index].end > y);
        while next < edges.len() && edges[next].start <= y {
            active.push(next);
            next += 1;
        }
        intersections.clear();
        let py = f64::from(y) + 0.5;
        for (i, &index) in active.iter().enumerate() {
            if i.is_multiple_of(256) {
                check_cancel(cancel)?;
            }
            let edge = &edges[index];
            intersections.push(edge.x + (py - edge.y) * edge.slope);
        }
        intersections.sort_by(f64::total_cmp);
        for span in intersections.as_chunks::<2>().0 {
            let start = (span[0] - 0.5).ceil().clamp(0.0, f64::from(width)) as u32;
            let end = (span[1] - 0.5).ceil().clamp(0.0, f64::from(width)) as u32;
            if end > start {
                pixels[y as usize * width as usize + start as usize
                    ..y as usize * width as usize + end as usize]
                    .fill(1);
                left = left.min(start);
                top = top.min(y);
                right = right.max(end);
                bottom = bottom.max(y + 1);
            }
        }
    }
    raster_selection(width, pixels, (left, top, right, bottom), cancel)
}
