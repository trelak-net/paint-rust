use crate::document::{Pixel, Selection, blend};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameters {
    pub dx: i32,
    pub dy: i32,
    pub copy: bool,
}

fn area(width: u32, height: u32, selection: Option<&Selection>) -> Selection {
    selection.cloned().unwrap_or(Selection {
        x: 0,
        y: 0,
        width,
        height,
        mask: None,
        ellipse: None,
    })
}

pub fn translated_selection(
    width: u32,
    height: u32,
    selection: Option<&Selection>,
    dx: i32,
    dy: i32,
) -> Option<Selection> {
    let area = area(width, height, selection);
    if (dx == 0 && dy == 0) || area.width == 0 || area.height == 0 {
        return selection.cloned();
    }
    let left = i64::from(area.x) + i64::from(dx);
    let top = i64::from(area.y) + i64::from(dy);
    let x = left.clamp(0, i64::from(width)) as u32;
    let y = top.clamp(0, i64::from(height)) as u32;
    let right = (left + i64::from(area.width)).clamp(0, i64::from(width)) as u32;
    let bottom = (top + i64::from(area.height)).clamp(0, i64::from(height)) as u32;
    (right > x && bottom > y).then_some(Selection {
        mask: area.mask.clone().map(|mut mask| {
            mask.x = mask.x.saturating_add(i64::from(dx));
            mask.y = mask.y.saturating_add(i64::from(dy));
            mask
        }),
        ellipse: area.ellipse.map(|mut ellipse| {
            ellipse.x = ellipse.x.saturating_add(i64::from(dx));
            ellipse.y = ellipse.y.saturating_add(i64::from(dy));
            ellipse
        }),
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}

pub fn render(
    source: &[Pixel],
    width: u32,
    height: u32,
    selection: Option<&Selection>,
    parameters: Parameters,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    let Parameters { dx, dy, copy } = parameters;
    if cancel.load(Ordering::Relaxed)
        || width == 0
        || height == 0
        || u64::from(width) * u64::from(height) != source.len() as u64
    {
        return None;
    }
    let area = area(width, height, selection);
    if area.x.checked_add(area.width)? > width || area.y.checked_add(area.height)? > height {
        return None;
    }
    let mut output = source.to_vec();
    if (dx == 0 && dy == 0) || area.width == 0 || area.height == 0 {
        return (!cancel.load(Ordering::Relaxed)).then_some(output);
    }
    if !copy {
        for y in area.y..area.y + area.height {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            let start = y as usize * width as usize + area.x as usize;
            for x in 0..area.width as usize {
                if x.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
                    return None;
                }
                if area.contains(area.x as usize + x, y as usize) {
                    output[start + x] = [0; 4];
                }
            }
        }
    }
    for sy in area.y..area.y + area.height {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        for sx in area.x..area.x + area.width {
            if sx.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            if !area.contains(sx as usize, sy as usize) {
                continue;
            }
            let x = i64::from(sx) + i64::from(dx);
            let y = i64::from(sy) + i64::from(dy);
            if x < 0 || y < 0 || x >= i64::from(width) || y >= i64::from(height) {
                continue;
            }
            let dst = y as usize * width as usize + x as usize;
            output[dst] = blend(
                output[dst],
                source[sy as usize * width as usize + sx as usize],
                255,
            );
        }
    }
    (!cancel.load(Ordering::Relaxed)).then_some(output)
}
