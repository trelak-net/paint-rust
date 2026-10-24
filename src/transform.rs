use crate::document::Pixel;

#[derive(Clone, Copy, PartialEq)]
pub enum Orientation {
    Clockwise,
    Counterclockwise,
    HalfTurn,
    FlipHorizontal,
    FlipVertical,
}

pub fn orient(source: &[Pixel], width: u32, height: u32, operation: Orientation) -> Vec<Pixel> {
    let new_width = if matches!(
        operation,
        Orientation::Clockwise | Orientation::Counterclockwise
    ) {
        height
    } else {
        width
    };
    let mut output = vec![[0; 4]; source.len()];
    for y in 0..height {
        for x in 0..width {
            let (dx, dy) = match operation {
                Orientation::Clockwise => (height - 1 - y, x),
                Orientation::Counterclockwise => (y, width - 1 - x),
                Orientation::HalfTurn => (width - 1 - x, height - 1 - y),
                Orientation::FlipHorizontal => (width - 1 - x, y),
                Orientation::FlipVertical => (x, height - 1 - y),
            };
            output[dy as usize * new_width as usize + dx as usize] =
                source[y as usize * width as usize + x as usize];
        }
    }
    output
}

pub fn canvas(source: &[Pixel], old: (u32, u32), new: (u32, u32), anchor: usize) -> Vec<Pixel> {
    let offset_x = ((i64::from(new.0) - i64::from(old.0)) * (anchor % 3) as i64).div_euclid(2);
    let offset_y = ((i64::from(new.1) - i64::from(old.1)) * (anchor / 3) as i64).div_euclid(2);
    let mut output = vec![[0; 4]; new.0 as usize * new.1 as usize];
    for y in 0..new.1 {
        for x in 0..new.0 {
            let sx = i64::from(x) - offset_x;
            let sy = i64::from(y) - offset_y;
            if sx >= 0 && sy >= 0 && sx < i64::from(old.0) && sy < i64::from(old.1) {
                output[y as usize * new.0 as usize + x as usize] =
                    source[sy as usize * old.0 as usize + sx as usize];
            }
        }
    }
    output
}

pub fn resize(source: &[Pixel], old: (u32, u32), new: (u32, u32), smooth: bool) -> Vec<Pixel> {
    if smooth && new.0 <= old.0 && new.1 <= old.1 && new != old {
        return downsample_area(source, old, new);
    }
    let mut output = vec![[0; 4]; new.0 as usize * new.1 as usize];
    for y in 0..new.1 {
        for x in 0..new.0 {
            let sx = ((x as f64 + 0.5) * f64::from(old.0) / f64::from(new.0) - 0.5)
                .clamp(0.0, f64::from(old.0 - 1));
            let sy = ((y as f64 + 0.5) * f64::from(old.1) / f64::from(new.1) - 0.5)
                .clamp(0.0, f64::from(old.1 - 1));
            let dst = &mut output[y as usize * new.0 as usize + x as usize];
            if !smooth {
                *dst = source[sy.round() as usize * old.0 as usize + sx.round() as usize];
                continue;
            }
            let x0 = sx.floor() as u32;
            let y0 = sy.floor() as u32;
            let x1 = (x0 + 1).min(old.0 - 1);
            let y1 = (y0 + 1).min(old.1 - 1);
            let tx = sx.fract();
            let ty = sy.fract();
            let mut rgba = [0.0; 4];
            for (px, py, weight) in [
                (x0, y0, (1.0 - tx) * (1.0 - ty)),
                (x1, y0, tx * (1.0 - ty)),
                (x0, y1, (1.0 - tx) * ty),
                (x1, y1, tx * ty),
            ] {
                let pixel = source[py as usize * old.0 as usize + px as usize];
                let alpha = f64::from(pixel[3]) * weight;
                rgba[3] += alpha;
                for c in 0..3 {
                    rgba[c] += f64::from(pixel[c]) * alpha;
                }
            }
            if rgba[3] > 0.0 {
                for c in 0..3 {
                    dst[c] = (rgba[c] / rgba[3]).round().clamp(0.0, 255.0) as u8;
                }
                dst[3] = rgba[3].round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    output
}

fn downsample_area(source: &[Pixel], old: (u32, u32), new: (u32, u32)) -> Vec<Pixel> {
    let mut output = vec![[0; 4]; new.0 as usize * new.1 as usize];
    let scale_x = f64::from(old.0) / f64::from(new.0);
    let scale_y = f64::from(old.1) / f64::from(new.1);
    for y in 0..new.1 {
        let top = f64::from(y) * scale_y;
        let bottom = f64::from(y + 1) * scale_y;
        for x in 0..new.0 {
            let left = f64::from(x) * scale_x;
            let right = f64::from(x + 1) * scale_x;
            let mut sum = [0.0; 4];
            for sy in top.floor() as u32..(bottom.ceil() as u32).min(old.1) {
                let weight_y = (bottom.min(f64::from(sy + 1)) - top.max(f64::from(sy))).max(0.0);
                for sx in left.floor() as u32..(right.ceil() as u32).min(old.0) {
                    let weight_x =
                        (right.min(f64::from(sx + 1)) - left.max(f64::from(sx))).max(0.0);
                    let pixel = source[sy as usize * old.0 as usize + sx as usize];
                    let alpha = f64::from(pixel[3]) * weight_x * weight_y;
                    sum[3] += alpha;
                    for c in 0..3 {
                        sum[c] += f64::from(pixel[c]) * alpha;
                    }
                }
            }
            if sum[3] > 0.0 {
                let pixel = &mut output[y as usize * new.0 as usize + x as usize];
                for c in 0..3 {
                    pixel[c] = (sum[c] / sum[3]).round().clamp(0.0, 255.0) as u8;
                }
                pixel[3] = (sum[3] / (scale_x * scale_y)).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    output
}
