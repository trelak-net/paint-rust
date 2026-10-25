use super::{AtomicBool, Ordering, Pixel};

pub(super) fn shadow(
    source: &[Pixel],
    width: u32,
    height: u32,
    radius: u32,
    offset: [i32; 2],
    color: Pixel,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    if radius > 64 || offset.into_iter().any(|v| !(-200..=200).contains(&v)) {
        return None;
    }
    if color[3] == 0 {
        return Some(source.to_vec());
    }
    // Only alpha carries shape information. Coloring after blur avoids RGB halos.
    let mut alpha = Vec::with_capacity(source.len());
    for chunk in source.chunks(4096) {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        alpha.extend(chunk.iter().map(|p| [255, 255, 255, p[3]]));
    }
    let blurred = super::blurs::gaussian(&alpha, width, height, radius, cancel)?;
    let mut output = source.to_vec();
    for y in 0..height {
        for x in 0..width {
            if x.is_multiple_of(256) && cancel.load(Ordering::Relaxed) {
                return None;
            }
            let sx = i64::from(x) - i64::from(offset[0]);
            let sy = i64::from(y) - i64::from(offset[1]);
            if sx < 0 || sy < 0 || sx >= i64::from(width) || sy >= i64::from(height) {
                continue;
            }
            let opacity = ((u32::from(blurred[sy as usize * width as usize + sx as usize][3])
                * u32::from(color[3])
                + 127)
                / 255) as u8;
            if opacity == 0 {
                continue;
            }
            let index = (y * width + x) as usize;
            output[index] =
                crate::document::blend([color[0], color[1], color[2], opacity], source[index], 255);
        }
    }
    Some(output)
}
