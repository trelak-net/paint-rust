use crate::document::{Pixel, Selection, blend};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FreeTransform {
    pub scale_x: f32,
    pub scale_y: f32,
    pub angle: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub flip_x: bool,
    pub flip_y: bool,
    pub copy: bool,
    pub smooth: bool,
}
impl Default for FreeTransform {
    fn default() -> Self {
        Self {
            scale_x: 1.0,
            scale_y: 1.0,
            angle: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            flip_x: false,
            flip_y: false,
            copy: false,
            smooth: true,
        }
    }
}

pub fn render(
    source: &[Pixel],
    width: u32,
    height: u32,
    selection: Option<&Selection>,
    parameters: FreeTransform,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    let p = parameters;
    if cancel.load(Ordering::Relaxed)
        || width == 0
        || height == 0
        || u64::from(width) * u64::from(height) != source.len() as u64
        || ![p.scale_x, p.scale_y, p.angle, p.offset_x, p.offset_y]
            .iter()
            .all(|v| v.is_finite())
        || !(0.01..=64.0).contains(&p.scale_x)
        || !(0.01..=64.0).contains(&p.scale_y)
        || p.offset_x.abs() > 1e6
        || p.offset_y.abs() > 1e6
    {
        return None;
    }
    if (p.scale_x == 1.0
        && p.scale_y == 1.0
        && p.angle.rem_euclid(360.0) == 0.0
        && p.offset_x == 0.0
        && p.offset_y == 0.0
        && !p.flip_x
        && !p.flip_y)
        || selection.is_some_and(|s| s.width == 0 || s.height == 0)
    {
        return Some(source.to_vec());
    }
    let (cx, cy) = selection
        .map(|s| {
            (
                f64::from(s.x) + f64::from(s.width) / 2.0,
                f64::from(s.y) + f64::from(s.height) / 2.0,
            )
        })
        .unwrap_or((f64::from(width) / 2.0, f64::from(height) / 2.0));
    let (sin, cos) = f64::from(p.angle).to_radians().sin_cos();
    let scale_x = f64::from(p.scale_x) * if p.flip_x { -1.0 } else { 1.0 };
    let scale_y = f64::from(p.scale_y) * if p.flip_y { -1.0 } else { 1.0 };
    let selected = |x: usize, y: usize| selection.is_none_or(|s| s.contains(x, y));
    let sample = |x: i64, y: i64| {
        if x < 0
            || y < 0
            || x >= i64::from(width)
            || y >= i64::from(height)
            || !selected(x as usize, y as usize)
        {
            [0; 4]
        } else {
            source[y as usize * width as usize + x as usize]
        }
    };
    let mut output = source.to_vec();
    for y in 0..height as usize {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        for x in 0..width as usize {
            let index = y * width as usize + x;
            if !p.copy && selected(x, y) {
                output[index] = [0; 4];
            }
            let dx = x as f64 + 0.5 - cx - f64::from(p.offset_x);
            let dy = y as f64 + 0.5 - cy - f64::from(p.offset_y);
            let sx = (cos * dx + sin * dy) / scale_x + cx;
            let sy = (-sin * dx + cos * dy) / scale_y + cy;
            let pixel = if p.smooth {
                let fx = sx - 0.5;
                let fy = sy - 0.5;
                let (x0, y0) = (fx.floor() as i64, fy.floor() as i64);
                let (tx, ty) = (fx - fx.floor(), fy - fy.floor());
                let mut sum = [0.0; 4];
                for (x, y, weight) in [
                    (x0, y0, (1.0 - tx) * (1.0 - ty)),
                    (x0 + 1, y0, tx * (1.0 - ty)),
                    (x0, y0 + 1, (1.0 - tx) * ty),
                    (x0 + 1, y0 + 1, tx * ty),
                ] {
                    let value = sample(x, y);
                    let alpha = f64::from(value[3]) * weight;
                    sum[3] += alpha;
                    for channel in 0..3 {
                        sum[channel] += f64::from(value[channel]) * alpha;
                    }
                }
                let mut value = [0; 4];
                if sum[3] > 0.0 {
                    for channel in 0..3 {
                        value[channel] = (sum[channel] / sum[3]).round().clamp(0.0, 255.0) as u8;
                    }
                    value[3] = sum[3].round().clamp(0.0, 255.0) as u8;
                }
                value
            } else {
                sample(sx.floor() as i64, sy.floor() as i64)
            };
            output[index] = blend(output[index], pixel, 255);
        }
    }
    Some(output)
}
