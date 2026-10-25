use super::{AtomicBool, Effect, Ordering, Pixel};
pub(super) fn render(
    source: &[Pixel],
    width: u32,
    height: u32,
    effect: Effect,
    cancel: &AtomicBool,
) -> Option<Vec<Pixel>> {
    if !effect.valid_settings() {
        return None;
    }
    if matches!(
        effect,
        Effect::ReduceNoise { radius: 0, .. }
            | Effect::ReduceNoise { strength: 0.0, .. }
            | Effect::Relief { strength: 0.0, .. }
    ) {
        return Some(source.to_vec());
    }
    let (filtered, secondary) = match effect {
        Effect::ReduceNoise { radius, .. } => (
            super::stylize::median(source, width, height, radius, 50, cancel)?,
            None,
        ),
        Effect::Relief { angle, .. } => (
            super::stylize::edges(source, width, height, Some(angle), cancel)?,
            None,
        ),
        Effect::Outline { radius, .. } => (
            super::stylize::median(source, width, height, radius, 0, cancel)?,
            Some(super::stylize::median(
                source, width, height, radius, 100, cancel,
            )?),
        ),
        _ => return None,
    };
    let mut output = source.to_vec();
    for (i, (pixel, filter)) in source.iter().zip(&filtered).enumerate() {
        if i.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
            return None;
        }
        if pixel[3] == 0 {
            continue;
        }
        match effect {
            Effect::ReduceNoise { strength, .. } => {
                for c in 0..3 {
                    output[i][c] = (f64::from(pixel[c])
                        + (f64::from(filter[c]) - f64::from(pixel[c])) * f64::from(strength))
                    .round()
                    .clamp(0.0, 255.0) as u8;
                }
            }
            Effect::Relief { strength, .. } => {
                for c in 0..3 {
                    output[i][c] = (f64::from(pixel[c])
                        + (f64::from(filter[0]) - 128.0) * f64::from(strength))
                    .round()
                    .clamp(0.0, 255.0) as u8;
                }
            }
            Effect::Outline { intensity, .. } => {
                let high = secondary.as_ref().unwrap()[i];
                let difference = (0..3)
                    .map(|c| high[c].saturating_sub(filter[c]))
                    .max()
                    .unwrap();
                let value = (255.0 - f64::from(difference) * f64::from(intensity))
                    .round()
                    .clamp(0.0, 255.0) as u8;
                output[i] = [value, value, value, pixel[3]];
            }
            _ => unreachable!(),
        }
    }
    Some(output)
}
