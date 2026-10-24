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
