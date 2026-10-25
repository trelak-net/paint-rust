use crate::document::{Pixel, Selection};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Channel {
    pub input_black: u8,
    pub input_white: u8,
    pub gamma: f32,
    pub output_black: u8,
    pub output_white: u8,
}
impl Default for Channel {
    fn default() -> Self {
        Self {
            input_black: 0,
            input_white: 255,
            gamma: 1.0,
            output_black: 0,
            output_white: 255,
        }
    }
}
impl Channel {
    fn valid(self) -> bool {
        self.input_black < self.input_white
            && self.gamma.is_finite()
            && (0.1..=10.0).contains(&self.gamma)
    }
    fn map(self, value: u8) -> u8 {
        let normalized = ((f64::from(value) - f64::from(self.input_black))
            / f64::from(self.input_white - self.input_black))
        .clamp(0.0, 1.0);
        let value = f64::from(self.output_black)
            + (f64::from(self.output_white) - f64::from(self.output_black))
                * normalized.powf(1.0 / f64::from(self.gamma));
        value.round().clamp(0.0, 255.0) as u8
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Levels {
    /// Master RGB followed by independent red, green and blue channel settings.
    pub channels: [Channel; 4],
}
impl Levels {
    pub(crate) fn tables(self) -> Option<[[u8; 256]; 3]> {
        if self.channels.iter().any(|channel| !channel.valid()) {
            return None;
        }
        let mut tables = [[0; 256]; 3];
        for (channel, table) in tables.iter_mut().enumerate() {
            for (value, output) in table.iter_mut().enumerate() {
                *output = self.channels[channel + 1].map(self.channels[0].map(value as u8));
            }
        }
        Some(tables)
    }
}

pub(crate) fn automatic(
    source: &[Pixel],
    width: u32,
    selection: Option<&Selection>,
    clip_percent: f32,
    cancel: &AtomicBool,
) -> Option<Levels> {
    if !clip_percent.is_finite() || !(0.0..=5.0).contains(&clip_percent) {
        return None;
    }
    let histogram = histogram(source, width, selection, cancel)?;
    let count: u32 = histogram[0].iter().sum();
    let mut levels = Levels::default();
    if count == 0 {
        return Some(levels);
    }
    let clipped = (f64::from(count) * f64::from(clip_percent) / 100.0).floor() as u32;
    for (channel, histogram) in histogram.iter().enumerate() {
        let mut total = 0;
        let low = histogram
            .iter()
            .position(|n| {
                total += n;
                total > clipped
            })
            .unwrap();
        total = 0;
        let high = histogram
            .iter()
            .rposition(|n| {
                total += n;
                total > clipped
            })
            .unwrap();
        if low < high {
            levels.channels[channel + 1].input_black = low as u8;
            levels.channels[channel + 1].input_white = high as u8;
        }
    }
    Some(levels)
}

/// Counts selected, nontransparent pixels without alpha weighting.
pub fn histogram(
    source: &[Pixel],
    width: u32,
    selection: Option<&Selection>,
    cancel: &AtomicBool,
) -> Option<[[u32; 256]; 3]> {
    histogram_with_luminance(source, width, selection, cancel)
        .map(|bins| [bins[0], bins[1], bins[2]])
}

/// Red, green, blue and weighted encoded-RGB luminance bins.
pub fn histogram_with_luminance(
    source: &[Pixel],
    width: u32,
    selection: Option<&Selection>,
    cancel: &AtomicBool,
) -> Option<[[u32; 256]; 4]> {
    if width == 0
        || source.len() > 16_777_216
        || !source.len().is_multiple_of(width as usize)
        || cancel.load(Ordering::Relaxed)
    {
        return None;
    }
    let mut histogram = [[0_u32; 256]; 4];
    for (index, pixel) in source.iter().enumerate() {
        if index.is_multiple_of(4096) && cancel.load(Ordering::Relaxed) {
            return None;
        }
        if pixel[3] == 0
            || selection
                .is_some_and(|s| !s.contains(index % width as usize, index / width as usize))
        {
            continue;
        }
        for channel in 0..3 {
            histogram[channel][pixel[channel] as usize] += 1;
        }
        let luma = (0.2126 * f64::from(pixel[0])
            + 0.7152 * f64::from(pixel[1])
            + 0.0722 * f64::from(pixel[2]))
        .round() as usize;
        histogram[3][luma] += 1;
    }
    Some(histogram)
}
