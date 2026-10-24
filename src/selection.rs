use std::sync::Arc;

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
    crate::document::Document::validate_size(width, height)?;
    if mode == CombineMode::Replace {
        return Ok(candidate.clone());
    }
    let mut pixels = vec![0; width as usize * height as usize];
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for y in 0..height {
        for x in 0..width {
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
    if right == 0 || bottom == 0 {
        left = 0;
        top = 0;
    }
    let mask_width = (right - left).max(1);
    let mask_height = (bottom - top).max(1);
    let mut cropped = vec![0; mask_width as usize * mask_height as usize];
    if right > left && bottom > top {
        for y in top..bottom {
            let source = y as usize * width as usize + left as usize;
            let target = (y - top) as usize * mask_width as usize;
            cropped[target..target + mask_width as usize]
                .copy_from_slice(&pixels[source..source + mask_width as usize]);
        }
    }
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
