use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err(crate::files::CANCELLED.into())
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub ellipse: Option<EllipseSelection>,
    pub mask: Option<RasterSelection>,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EllipseSelection {
    pub x: i64,
    pub y: i64,
    pub width: u32,
    pub height: u32,
}

impl Selection {
    pub fn elliptical(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            mask: None,
            x,
            y,
            width,
            height,
            ellipse: Some(EllipseSelection {
                x: i64::from(x),
                y: i64::from(y),
                width,
                height,
            }),
        }
    }

    pub(crate) fn contains(&self, x: usize, y: usize) -> bool {
        if x < self.x as usize
            || y < self.y as usize
            || x >= self.x as usize + self.width as usize
            || y >= self.y as usize + self.height as usize
        {
            return false;
        }
        if let Some(mask) = &self.mask {
            let mx = x as i64 - mask.x;
            let my = y as i64 - mask.y;
            return mx >= 0
                && my >= 0
                && mx < i64::from(mask.width)
                && my < i64::from(mask.height)
                && mask
                    .pixels
                    .get(my as usize * mask.width as usize + mx as usize)
                    .is_some_and(|value| *value != 0);
        }
        self.ellipse.is_none_or(|ellipse| {
            if ellipse.width == 0 || ellipse.height == 0 {
                return false;
            }
            let nx = 2.0 * (x as f64 + 0.5 - ellipse.x as f64) / f64::from(ellipse.width) - 1.0;
            let ny = 2.0 * (y as f64 + 0.5 - ellipse.y as f64) / f64::from(ellipse.height) - 1.0;
            nx * nx + ny * ny <= 1.0
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RasterSelection {
    pub x: i64,
    pub y: i64,
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombineMode {
    Replace,
    Add,
    Subtract,
    Intersect,
}
impl CombineMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Replace => "Replace",
            Self::Add => "Add",
            Self::Subtract => "Subtract",
            Self::Intersect => "Intersect",
        }
    }
}

pub fn combine(
    width: u32,
    height: u32,
    previous: Option<&Selection>,
    candidate: &Selection,
    mode: CombineMode,
) -> Result<Selection, String> {
    combine_cancellable(
        width,
        height,
        previous,
        candidate,
        mode,
        &AtomicBool::new(false),
    )
}

pub fn combine_cancellable(
    width: u32,
    height: u32,
    previous: Option<&Selection>,
    candidate: &Selection,
    mode: CombineMode,
    cancel: &AtomicBool,
) -> Result<Selection, String> {
    check_cancel(cancel)?;
    crate::document::Document::validate_size(width, height)?;
    if mode == CombineMode::Replace {
        return Ok(candidate.clone());
    }
    let mut pixels = vec![0; width as usize * height as usize];
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for y in 0..height {
        check_cancel(cancel)?;
        for x in 0..width {
            if x.is_multiple_of(4096) {
                check_cancel(cancel)?;
            }
            let old = previous.map(|s| s.contains(x as usize, y as usize));
            let new = candidate.contains(x as usize, y as usize);
            let selected = match mode {
                CombineMode::Replace => new,
                CombineMode::Add => old.unwrap_or(false) || new,
                CombineMode::Subtract => old.unwrap_or(true) && !new,
                CombineMode::Intersect => old.unwrap_or(true) && new,
            };
            if selected {
                pixels[y as usize * width as usize + x as usize] = 1;
                left = left.min(x);
                top = top.min(y);
                right = right.max(x + 1);
                bottom = bottom.max(y + 1);
            }
        }
    }
    raster_selection(width, pixels, (left, top, right, bottom), cancel)
}

fn raster_selection(
    width: u32,
    pixels: Vec<u8>,
    bounds: (u32, u32, u32, u32),
    cancel: &AtomicBool,
) -> Result<Selection, String> {
    check_cancel(cancel)?;
    let (mut left, mut top, right, bottom) = bounds;
    if right == 0 || bottom == 0 {
        left = 0;
        top = 0;
    }
    let mask_width = (right - left).max(1);
    let mask_height = (bottom - top).max(1);
    let mut cropped = vec![0; mask_width as usize * mask_height as usize];
    if right > left && bottom > top {
        for y in top..bottom {
            check_cancel(cancel)?;
            let source = y as usize * width as usize + left as usize;
            let target = (y - top) as usize * mask_width as usize;
            cropped[target..target + mask_width as usize]
                .copy_from_slice(&pixels[source..source + mask_width as usize]);
        }
    }
    check_cancel(cancel)?;
    Ok(Selection {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
        ellipse: None,
        mask: Some(RasterSelection {
            x: i64::from(left),
            y: i64::from(top),
            width: mask_width,
            height: mask_height,
            pixels: Arc::new(cropped),
        }),
    })
}

pub fn polygon(width: u32, height: u32, points: &[(f64, f64)]) -> Result<Selection, String> {
    polygon_cancellable(width, height, points, &AtomicBool::new(false))
}

pub fn polygon_cancellable(
    width: u32,
    height: u32,
    points: &[(f64, f64)],
    cancel: &AtomicBool,
) -> Result<Selection, String> {
    check_cancel(cancel)?;
    crate::document::Document::validate_size(width, height)?;
    if points.len() < 3
        || points.len() > 4096
        || points
            .iter()
            .any(|(x, y)| !x.is_finite() || !y.is_finite() || x.abs() > 1e9 || y.abs() > 1e9)
    {
        return Err(
            "A polygon needs 3–4096 finite points within the supported coordinate range.".into(),
        );
    }
    let mut pixels = vec![0; width as usize * height as usize];
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    let mut intersections = Vec::with_capacity(points.len());
    for y in 0..height {
        check_cancel(cancel)?;
        let py = f64::from(y) + 0.5;
        intersections.clear();
        for index in 0..points.len() {
            let (ax, ay) = points[index];
            let (bx, by) = points[(index + 1) % points.len()];
            if (ay > py) != (by > py) {
                intersections.push(ax + (py - ay) * (bx - ax) / (by - ay));
            }
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

/// Select colors by maximum premultiplied RGBA channel difference (0–255).
/// Connected mode follows four-neighbor connectivity; global mode samples all pixels.
pub fn color_cancellable(
    width: u32,
    height: u32,
    pixels: &[crate::document::Pixel],
    seed: (u32, u32),
    tolerance: u8,
    contiguous: bool,
    cancel: &AtomicBool,
) -> Result<Selection, String> {
    check_cancel(cancel)?;
    crate::document::Document::validate_size(width, height)?;
    if pixels.len() != width as usize * height as usize || seed.0 >= width || seed.1 >= height {
        return Err("Invalid color-selection image or seed.".into());
    }
    let premultiply = |p: crate::document::Pixel| -> [u8; 4] {
        [
            ((u16::from(p[0]) * u16::from(p[3]) + 127) / 255) as u8,
            ((u16::from(p[1]) * u16::from(p[3]) + 127) / 255) as u8,
            ((u16::from(p[2]) * u16::from(p[3]) + 127) / 255) as u8,
            p[3],
        ]
    };
    let start = seed.1 as usize * width as usize + seed.0 as usize;
    let reference = premultiply(pixels[start]);
    let matches = |index: usize| {
        premultiply(pixels[index])
            .iter()
            .zip(reference)
            .all(|(a, b)| a.abs_diff(b) <= tolerance)
    };
    let mut mask = vec![0_u8; pixels.len()];
    if contiguous {
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start as u32);
        mask[start] = 1;
        let mut steps = 0;
        while let Some(index) = queue.pop_front() {
            if steps % 4096 == 0 {
                check_cancel(cancel)?;
            }
            steps += 1;
            let index = index as usize;
            let x = index % width as usize;
            let neighbors = [
                (x > 0).then(|| index - 1),
                (x + 1 < width as usize).then_some(index + 1),
                (index >= width as usize).then(|| index - width as usize),
                (index + (width as usize) < pixels.len()).then_some(index + width as usize),
            ];
            for next in neighbors.into_iter().flatten() {
                if mask[next] == 0 {
                    mask[next] = if matches(next) { 1 } else { 2 };
                    if mask[next] == 1 {
                        queue.push_back(next as u32);
                    }
                }
            }
        }
    } else {
        for (index, value) in mask.iter_mut().enumerate() {
            if index % 4096 == 0 {
                check_cancel(cancel)?;
            }
            *value = u8::from(matches(index));
        }
    }
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for (index, value) in mask.iter_mut().enumerate() {
        if index % 4096 == 0 {
            check_cancel(cancel)?;
        }
        if *value == 1 {
            let x = (index % width as usize) as u32;
            let y = (index / width as usize) as u32;
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        } else {
            *value = 0;
        }
    }
    raster_selection(width, mask, (left, top, right, bottom), cancel)
}
