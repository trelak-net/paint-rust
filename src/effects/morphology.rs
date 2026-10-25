use super::{AtomicBool, Ordering, Pixel};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MorphologyOperation {
    Dilate,
    Erode,
    Open,
    Close,
}
impl MorphologyOperation {
    pub const ALL: [Self; 4] = [Self::Dilate, Self::Erode, Self::Open, Self::Close];
    pub fn label(self) -> &'static str {
        match self {
            Self::Dilate => "Dilate",
            Self::Erode => "Erode",
            Self::Open => "Open",
            Self::Close => "Close",
        }
    }
}

fn pass(
    source: &[[u16; 4]],
    width: usize,
    height: usize,
    radius: usize,
    vertical: bool,
    maximum: bool,
    cancel: &AtomicBool,
) -> Option<Vec<[u16; 4]>> {
    let mut output = vec![[0; 4]; source.len()];
    let (lines, length) = if vertical {
        (width, height)
    } else {
        (height, width)
    };
    for line in 0..lines {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let index = |position: usize| {
            if vertical {
                position * width + line
            } else {
                line * width + position
            }
        };
        let mut queues: [VecDeque<(usize, u16)>; 4] =
            std::array::from_fn(|_| VecDeque::with_capacity(radius * 2 + 1));
        for step in 0..length + 2 * radius {
            if step.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let pixel = source[index(step.saturating_sub(radius).min(length - 1))];
            for c in 0..4 {
                let queue = &mut queues[c];
                while queue
                    .front()
                    .is_some_and(|&(position, _)| position + 2 * radius < step)
                {
                    queue.pop_front();
                }
                while queue.back().is_some_and(|&(_, value)| {
                    if maximum {
                        value <= pixel[c]
                    } else {
                        value >= pixel[c]
                    }
                }) {
                    queue.pop_back();
                }
                queue.push_back((step, pixel[c]));
                if step >= 2 * radius {
                    output[index(step - 2 * radius)][c] = queue.front().unwrap().1;
                }
            }
        }
    }
    Some(output)
}

pub(super) fn render(
    source: &[Pixel],
    width: u32,
    height: u32,
    radius: u32,
    operation: MorphologyOperation,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    if radius > 64 || cancel.load(Ordering::Relaxed) {
        return None;
    }
    if radius == 0 {
        return Some(source.to_vec());
    }
    let mut pixels = Vec::with_capacity(source.len());
    for (i, pixel) in source.iter().enumerate() {
        if i.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
            return None;
        }
        let a = u16::from(pixel[3]);
        pixels.push([
            u16::from(pixel[0]) * a,
            u16::from(pixel[1]) * a,
            u16::from(pixel[2]) * a,
            a * 255,
        ]);
    }
    let operations: &[bool] = match operation {
        MorphologyOperation::Dilate => &[true],
        MorphologyOperation::Erode => &[false],
        MorphologyOperation::Open => &[false, true],
        MorphologyOperation::Close => &[true, false],
    };
    for &maximum in operations {
        for vertical in [false, true] {
            pixels = pass(
                &pixels,
                width as usize,
                height as usize,
                radius as usize,
                vertical,
                maximum,
                cancel,
            )?;
        }
    }
    let mut output = vec![[0; 4]; source.len()];
    for (i, (pixel, dest)) in pixels.into_iter().zip(&mut output).enumerate() {
        if i.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
            return None;
        }
        let alpha = u32::from(pixel[3]);
        for c in 0..3 {
            dest[c] = (u32::from(pixel[c]) * 255 + alpha / 2)
                .checked_div(alpha)
                .unwrap_or(0)
                .min(255) as u8;
        }
        dest[3] = ((alpha + 127) / 255) as u8;
    }
    Some(output)
}
