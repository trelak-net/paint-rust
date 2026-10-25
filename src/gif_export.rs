//! Single-frame GIF encoding with optional serpentine error diffusion.
use std::{
    io::Write,
    sync::atomic::{AtomicBool, Ordering},
};

struct Node {
    rgb: [u8; 3],
    index: u8,
    axis: usize,
    children: [Option<Box<Node>>; 2],
}
impl Node {
    fn build(colors: &mut [([u8; 3], u8)], depth: usize) -> Option<Box<Self>> {
        if colors.is_empty() {
            return None;
        }
        let axis = depth % 3;
        colors.sort_unstable_by_key(|&(rgb, index)| (rgb[axis], index));
        let middle = colors.len() / 2;
        let (rgb, index) = colors[middle];
        let (left, right) = colors.split_at_mut(middle);
        Some(Box::new(Self {
            rgb,
            index,
            axis,
            children: [
                Self::build(left, depth + 1),
                Self::build(&mut right[1..], depth + 1),
            ],
        }))
    }
    fn nearest(&self, rgb: [u8; 3], best: &mut (u32, u8)) {
        let distance = (0..3)
            .map(|c| (i32::from(rgb[c]) - i32::from(self.rgb[c])).pow(2) as u32)
            .sum();
        *best = (*best).min((distance, self.index));
        let delta = i32::from(rgb[self.axis]) - i32::from(self.rgb[self.axis]);
        let near = usize::from(delta >= 0);
        if let Some(child) = &self.children[near] {
            child.nearest(rgb, best);
        }
        if delta.pow(2) as u32 <= best.0
            && let Some(child) = &self.children[1 - near]
        {
            child.nearest(rgb, best);
        }
    }
}

pub(crate) fn encode(
    mut rgba: Vec<u8>,
    size: (u32, u32),
    writer: impl Write,
    dither: bool,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let mut frame = gif::Frame::from_rgba_speed(size.0 as u16, size.1 as u16, &mut rgba, 1);
    if cancel.load(Ordering::Relaxed) {
        return Err(crate::files::CANCELLED.into());
    }
    if dither {
        let palette = frame.palette.as_ref().ok_or("Missing GIF palette")?;
        let mut colors: Vec<_> = palette
            .as_chunks::<3>()
            .0
            .iter()
            .enumerate()
            .filter(|(index, _)| Some(*index as u8) != frame.transparent)
            .map(|(index, rgb)| ([rgb[0], rgb[1], rgb[2]], index as u8))
            .collect();
        if let Some(tree) = Node::build(&mut colors, 0) {
            let width = size.0 as usize;
            let mut current = vec![[0.0_f32; 3]; width + 2];
            let mut next = current.clone();
            let indices = frame.buffer.to_mut();
            for y in 0..size.1 as usize {
                if cancel.load(Ordering::Relaxed) {
                    return Err(crate::files::CANCELLED.into());
                }
                let forward = y % 2 == 0;
                for column in 0..width {
                    if column.is_multiple_of(1024) && cancel.load(Ordering::Relaxed) {
                        return Err(crate::files::CANCELLED.into());
                    }
                    let x = if forward { column } else { width - 1 - column };
                    let pixel = &rgba[4 * (y * width + x)..][..4];
                    if pixel[3] == 0 {
                        continue;
                    }
                    let target: [f32; 3] = std::array::from_fn(|c| {
                        (f32::from(pixel[c]) + current[x + 1][c]).clamp(0.0, 255.0)
                    });
                    let mut best = (u32::MAX, 255);
                    tree.nearest(target.map(|c| c.round() as u8), &mut best);
                    indices[y * width + x] = best.1;
                    let (ahead, behind) = if forward { (x + 2, x) } else { (x, x + 2) };
                    for c in 0..3 {
                        let error = target[c] - f32::from(palette[usize::from(best.1) * 3 + c]);
                        current[ahead][c] += error * (7.0 / 16.0);
                        next[behind][c] += error * (3.0 / 16.0);
                        next[x + 1][c] += error * (5.0 / 16.0);
                        next[ahead][c] += error * (1.0 / 16.0);
                    }
                }
                std::mem::swap(&mut current, &mut next);
                next.fill([0.0; 3]);
            }
        }
    }
    let mut encoder =
        gif::Encoder::new(writer, frame.width, frame.height, &[]).map_err(|e| e.to_string())?;
    encoder.write_frame(&frame).map_err(|e| e.to_string())?;
    encoder.into_inner().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn palette_search_matches_exhaustive_distance_including_ties() {
        let mut colors: Vec<_> = (0..=255_u8)
            .map(|i| ([i, i.wrapping_mul(37), i.wrapping_mul(91)], i))
            .collect();
        let tree = Node::build(&mut colors, 0).unwrap();
        for r in (0..=255).step_by(17) {
            for g in (0..=255).step_by(17) {
                for b in (0..=255).step_by(17) {
                    let target = [r, g, b];
                    let expected = colors
                        .iter()
                        .map(|(rgb, index)| {
                            (
                                (0..3)
                                    .map(|c| {
                                        (i32::from(target[c]) - i32::from(rgb[c])).pow(2) as u32
                                    })
                                    .sum(),
                                *index,
                            )
                        })
                        .min()
                        .unwrap();
                    let mut actual = (u32::MAX, 255);
                    tree.nearest(target, &mut actual);
                    assert_eq!(actual, expected);
                }
            }
        }
    }
}
