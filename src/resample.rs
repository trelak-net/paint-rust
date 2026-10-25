//! Separable alpha-aware resizing with a bounded cache of filtered rows.
use crate::{
    document::{Document, Pixel},
    files::CANCELLED,
};
use std::{
    collections::{HashMap, VecDeque},
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Filter {
    Nearest,
    BilinearLowQuality,
    Bilinear,
    #[default]
    Bicubic,
    BicubicSmooth,
    Lanczos,
    Area,
    Adaptive,
}
impl Filter {
    pub const ALL: [Self; 8] = [
        Self::Nearest,
        Self::BilinearLowQuality,
        Self::Bilinear,
        Self::Bicubic,
        Self::BicubicSmooth,
        Self::Lanczos,
        Self::Area,
        Self::Adaptive,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Nearest => "Nearest neighbor",
            Self::BilinearLowQuality => "Bilinear (low quality)",
            Self::Bilinear => "Bilinear",
            Self::Bicubic => "Bicubic",
            Self::BicubicSmooth => "Bicubic (smooth)",
            Self::Lanczos => "Lanczos 3",
            Self::Area => "Area / bilinear",
            Self::Adaptive => "Adaptive",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub filter: Filter,
    pub gamma_correct: bool,
}
fn check(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}
fn kernel(filter: Filter, x: f64) -> f64 {
    let x = x.abs();
    match filter {
        Filter::Bilinear | Filter::BilinearLowQuality => (1.0 - x).max(0.0),
        Filter::Bicubic => {
            if x < 1.0 {
                1.5 * x * x * x - 2.5 * x * x + 1.0
            } else if x < 2.0 {
                -0.5 * x * x * x + 2.5 * x * x - 4.0 * x + 2.0
            } else {
                0.0
            }
        }
        Filter::BicubicSmooth => {
            if x < 1.0 {
                (4.0 - 6.0 * x * x + 3.0 * x * x * x) / 6.0
            } else if x < 2.0 {
                (2.0 - x).powi(3) / 6.0
            } else {
                0.0
            }
        }
        Filter::Lanczos => {
            if x < 1e-12 {
                1.0
            } else if x < 3.0 {
                let p = std::f64::consts::PI * x;
                (p.sin() / p) * ((p / 3.0).sin() / (p / 3.0))
            } else {
                0.0
            }
        }
        _ => unreachable!(),
    }
}
fn weights(old: u32, new: u32, filter: Filter) -> Vec<Vec<(usize, f64)>> {
    let ratio = f64::from(old) / f64::from(new);
    let filter = match filter {
        Filter::Adaptive => {
            if ratio > 1.0 {
                Filter::Lanczos
            } else {
                Filter::Bicubic
            }
        }
        Filter::Area if ratio <= 1.0 => Filter::Bilinear,
        other => other,
    };
    (0..new)
        .map(|target| {
            if filter == Filter::Area {
                let left = f64::from(target) * ratio;
                let right = f64::from(target + 1) * ratio;
                return (left.floor() as u32..right.ceil().min(f64::from(old)) as u32)
                    .map(|i| {
                        (
                            i as usize,
                            (right.min(f64::from(i + 1)) - left.max(f64::from(i))).max(0.0) / ratio,
                        )
                    })
                    .collect();
            }
            let center = (f64::from(target) + 0.5) * ratio - 0.5;
            let scale = if filter == Filter::BilinearLowQuality {
                1.0
            } else {
                ratio.max(1.0)
            };
            let radius = match filter {
                Filter::Bicubic | Filter::BicubicSmooth => 2.0,
                Filter::Lanczos => 3.0,
                _ => 1.0,
            } * scale;
            let mut row: Vec<_> = ((center - radius).ceil() as i64
                ..=(center + radius).floor() as i64)
                .filter_map(|i| {
                    let weight = kernel(filter, (i as f64 - center) / scale);
                    (weight.abs() > 1e-15)
                        .then_some((i.clamp(0, i64::from(old) - 1) as usize, weight))
                })
                .collect();
            let sum: f64 = row.iter().map(|(_, w)| w).sum();
            for (_, weight) in &mut row {
                *weight /= sum;
            }
            row
        })
        .collect()
}
fn linear(value: f64) -> f64 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
fn srgb(value: f64) -> f64 {
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

pub fn resize(
    source: &[Pixel],
    old: (u32, u32),
    new: (u32, u32),
    options: Options,
    cancel: &AtomicBool,
) -> Result<Vec<Pixel>, String> {
    check(cancel)?;
    Document::validate_size(old.0, old.1)?;
    Document::validate_size(new.0, new.1)?;
    if source.len() != old.0 as usize * old.1 as usize {
        return Err("Invalid resize source buffer".into());
    }
    if old == new {
        return Ok(source.to_vec());
    }
    if options.filter == Filter::Nearest {
        return crate::transform::resize_cancellable(source, old, new, false, cancel)
            .ok_or_else(|| CANCELLED.into());
    }
    let transposed = u64::from(new.1) * u64::from(old.0) < u64::from(new.0) * u64::from(old.1);
    let (old_a, old_b, new_a, new_b) = if transposed {
        (old.1, old.0, new.1, new.0)
    } else {
        (old.0, old.1, new.0, new.1)
    };
    let wa = weights(old_a, new_a, options.filter);
    let wb = weights(old_b, new_b, options.filter);
    let lut: [f64; 256] = std::array::from_fn(|i| {
        let v = i as f64 / 255.0;
        if options.gamma_correct { linear(v) } else { v }
    });
    let mut cache: HashMap<usize, Vec<[f32; 4]>> = HashMap::new();
    let mut order = VecDeque::new();
    let cache_rows = (32 * 1024 * 1024 / (new_a as usize * 16)).max(1);
    let mut output = vec![[0; 4]; new.0 as usize * new.1 as usize];
    for (b, contributions) in wb.iter().enumerate() {
        check(cancel)?;
        let mut accum = vec![[0.0f64; 4]; new_a as usize];
        for &(source_b, weight_b) in contributions {
            check(cancel)?;
            if !cache.contains_key(&source_b) {
                while cache.len() >= cache_rows {
                    if let Some(key) = order.pop_front() {
                        cache.remove(&key);
                    }
                }
                let mut row = Vec::with_capacity(new_a as usize);
                for (a, weights_a) in wa.iter().enumerate() {
                    if a.is_multiple_of(256) {
                        check(cancel)?;
                    }
                    let mut value = [0.0f64; 4];
                    for &(source_a, weight_a) in weights_a {
                        let index = if transposed {
                            source_a * old.0 as usize + source_b
                        } else {
                            source_b * old.0 as usize + source_a
                        };
                        let pixel = source[index];
                        let alpha = f64::from(pixel[3]) / 255.0;
                        for c in 0..3 {
                            value[c] += lut[pixel[c] as usize] * alpha * weight_a;
                        }
                        value[3] += alpha * weight_a;
                    }
                    row.push(value.map(|v| v as f32));
                }
                cache.insert(source_b, row);
                order.push_back(source_b);
            }
            for (value, filtered) in accum.iter_mut().zip(&cache[&source_b]) {
                for c in 0..4 {
                    value[c] += f64::from(filtered[c]) * weight_b;
                }
            }
        }
        for (a, value) in accum.into_iter().enumerate() {
            let alpha = value[3].clamp(0.0, 1.0);
            let mut pixel = [0; 4];
            pixel[3] = (alpha * 255.0).round() as u8;
            if pixel[3] > 0 && value[3] > 0.0 {
                for c in 0..3 {
                    let v = (value[c] / value[3]).clamp(0.0, 1.0);
                    pixel[c] =
                        ((if options.gamma_correct { srgb(v) } else { v }) * 255.0).round() as u8;
                }
            }
            let index = if transposed {
                a * new.0 as usize + b
            } else {
                b * new.0 as usize + a
            };
            output[index] = pixel;
        }
    }
    check(cancel)?;
    Ok(output)
}
