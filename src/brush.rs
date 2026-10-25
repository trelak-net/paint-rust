//! Coverage-based round strokes. Coverage is accumulated once per transaction.
pub(crate) mod spacing;
use crate::document::{BlendMode, Pixel, Selection};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// Dab separation as a percentage of diameter; zero draws continuous segments.
    pub spacing: f32,
    pub blend_mode: BlendMode,
    pub hardness: f32,
    pub opacity: f32,
    pub antialias: bool,
    pub blend: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            spacing: 0.0,
            blend_mode: BlendMode::Normal,
            hardness: 1.0,
            opacity: 1.0,
            antialias: false,
            blend: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paint {
    Color(Pixel),
    Erase,
    /// Sample the frozen clone source, or the destination before the stroke.
    Clone {
        offset: (i32, i32),
    },
    /// Shift matching RGBA values toward the target while preserving shade differences.
    Recolor {
        /// Palette basis for edge feathering; None uses the sampled color.
        edge_basis: Option<Pixel>,
        alpha: crate::selection::AlphaMatch,
        from: Pixel,
        to: Pixel,
        tolerance: u8,
    },
}

pub(crate) struct Segment<'a> {
    pub clone_source: Option<&'a [Pixel]>,
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub diameter: f32,
    pub paint: Paint,
    pub options: Options,
}

pub(crate) fn render(
    pixels: &mut [Pixel],
    source: &[Pixel],
    coverage: &mut [u8],
    dimensions: (u32, u32),
    selection: Option<&Selection>,
    segment: Segment<'_>,
    cancel: &std::sync::atomic::AtomicBool,
) {
    let Segment {
        clone_source,
        from,
        to,
        diameter,
        paint,
        options,
    } = segment;
    if [
        from.0,
        from.1,
        to.0,
        to.1,
        diameter,
        options.hardness,
        options.opacity,
    ]
    .iter()
    .any(|v| !v.is_finite())
    {
        return;
    }
    let (width, height) = dimensions;
    let radius = f64::from(diameter.clamp(1.0, 4096.0)) / 2.0;
    let fringe = if options.antialias { 0.5 } else { 0.0 };
    let band = radius + fringe;
    let (fx, fy) = (f64::from(from.0), f64::from(from.1));
    let (dx, dy) = (f64::from(to.0) - fx, f64::from(to.1) - fy);
    let length = dx * dx + dy * dy;
    let min_y = (fy.min(f64::from(to.1)) - band).floor().max(0.0) as u32;
    let max_y = (fy.max(f64::from(to.1)) + band)
        .ceil()
        .clamp(0.0, f64::from(height)) as u32;
    for y in min_y..max_y {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let py = f64::from(y) + 0.5;
        let (a, b) = if dy.abs() > f64::EPSILON {
            (
                ((py - fy - band) / dy).clamp(0.0, 1.0),
                ((py - fy + band) / dy).clamp(0.0, 1.0),
            )
        } else {
            (0.0, 1.0)
        };
        let (ax, bx) = (fx + a * dx, fx + b * dx);
        let min_x = (ax.min(bx) - band).floor().max(0.0) as u32;
        let max_x = (ax.max(bx) + band).ceil().clamp(0.0, f64::from(width)) as u32;
        for x in min_x..max_x {
            if x.is_multiple_of(256) && cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            if selection.is_some_and(|s| !s.contains(x as usize, y as usize)) {
                continue;
            }
            let px = f64::from(x) + 0.5;
            let t = if length > 0.0 {
                ((px - fx) * dx + (py - fy) * dy) / length
            } else {
                0.0
            }
            .clamp(0.0, 1.0);
            let distance = (px - fx - t * dx).hypot(py - fy - t * dy);
            let edge = if options.antialias {
                (radius + 0.5 - distance).clamp(0.0, 1.0)
            } else {
                f64::from(distance <= radius)
            };
            let inner = radius * f64::from(options.hardness.clamp(0.0, 1.0));
            let softness = if options.antialias && inner < radius {
                ((radius - distance) / (radius - inner)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let mut amount = (255.0 * edge * softness * f64::from(options.opacity.clamp(0.0, 1.0)))
                .round() as u8;
            let i = y as usize * width as usize + x as usize;
            if amount <= coverage[i] {
                continue;
            }
            let (color, blend, erase) = match paint {
                Paint::Color(color) => (color, options.blend, false),
                Paint::Erase => ([0; 4], false, true),
                Paint::Clone { offset } => {
                    let sx = i64::from(x) + i64::from(offset.0);
                    let sy = i64::from(y) + i64::from(offset.1);
                    if sx < 0 || sy < 0 || sx >= i64::from(width) || sy >= i64::from(height) {
                        continue;
                    }
                    (
                        clone_source.unwrap_or(source)[sy as usize * width as usize + sx as usize],
                        options.blend,
                        false,
                    )
                }
                Paint::Recolor {
                    edge_basis,
                    alpha,
                    from,
                    to,
                    tolerance,
                } => {
                    let mask = recolor_coverage(
                        source,
                        dimensions,
                        (x, y),
                        alpha,
                        (from, edge_basis.unwrap_or(from)),
                        tolerance,
                        options.antialias,
                    );
                    amount = scale_byte(amount, mask);
                    if amount <= coverage[i] {
                        continue;
                    }
                    let shifted = std::array::from_fn(|c| {
                        (i16::from(source[i][c]) + i16::from(to[c]) - i16::from(from[c]))
                            .clamp(0, 255) as u8
                    });
                    (shifted, false, false)
                }
            };
            coverage[i] = amount;
            pixels[i] = if matches!(paint, Paint::Recolor { .. }) {
                crate::document::merge_pixel(source[i], color, amount, BlendMode::Normal)
            } else if blend && !erase && options.blend_mode != BlendMode::Normal {
                crate::document::blend_layer(source[i], color, amount, options.blend_mode)
            } else {
                composite(source[i], color, amount, blend, erase)
            };
        }
    }
}

pub(crate) fn composite(dst: Pixel, src: Pixel, amount: u8, blend: bool, erase: bool) -> Pixel {
    if amount == 0 {
        return dst;
    }
    let coverage = f64::from(amount) / 255.0;
    if erase {
        let mut output = dst;
        output[3] = (f64::from(dst[3]) * (1.0 - coverage)).round() as u8;
        return output;
    }
    if !blend && amount == 255 {
        return src;
    }
    let source_alpha = f64::from(src[3]) / 255.0;
    let destination_alpha = f64::from(dst[3]) / 255.0;
    let a = source_alpha * coverage;
    let b = destination_alpha * (1.0 - if blend { a } else { coverage });
    let alpha = a + b;
    if alpha <= 0.0 {
        return [0; 4];
    }
    let mut output = [0; 4];
    for channel in 0..3 {
        output[channel] =
            ((f64::from(src[channel]) * a + f64::from(dst[channel]) * b) / alpha).round() as u8;
    }
    output[3] = (alpha * 255.0).round() as u8;
    output
}

fn scale_byte(a: u8, b: u8) -> u8 {
    ((u16::from(a) * u16::from(b) + 127) / 255) as u8
}

/// Extend a color region by one pixel using neighboring coverage and color distance.
fn recolor_coverage(
    source: &[Pixel],
    dimensions: (u32, u32),
    point: (u32, u32),
    alpha: crate::selection::AlphaMatch,
    basis: (Pixel, Pixel),
    tolerance: u8,
    antialias: bool,
) -> u8 {
    let (width, height) = dimensions;
    let (x, y) = point;
    let distance = alpha.distance(source[(y * width + x) as usize], basis.0);
    if distance <= tolerance {
        return 255;
    }
    if !antialias {
        return 0;
    }
    // Four outer corner sectors and four inner corner sectors. Orthogonal
    // neighbors cover both sectors on their side; diagonal neighbors only one.
    let neighbors = [
        (-1, -1, 0x01),
        (0, -1, 0x33),
        (1, -1, 0x02),
        (-1, 0, 0x55),
        (1, 0, 0xaa),
        (-1, 1, 0x04),
        (0, 1, 0xcc),
        (1, 1, 0x08),
    ];
    let mut sectors = 0_u8;
    for (dx, dy, bits) in neighbors {
        let (nx, ny) = (i64::from(x) + dx, i64::from(y) + dy);
        if nx >= 0
            && ny >= 0
            && nx < i64::from(width)
            && ny < i64::from(height)
            && alpha.matches(
                source[ny as usize * width as usize + nx as usize],
                basis.0,
                tolerance,
            )
        {
            sectors |= bits;
        }
    }
    if sectors == 0 {
        return 0;
    }
    let spatial = (sectors.count_ones() * 255 / 8) as u8;
    let distance = alpha.distance(source[(y * width + x) as usize], basis.1);
    let color = (255.0
        * (1.0
            - (f32::from(distance.saturating_sub(tolerance)) / f32::from(255 - tolerance)).sqrt()))
    .round() as u8;
    scale_byte(
        255 - scale_byte(255 - spatial, 255 - spatial),
        255 - scale_byte(255 - color, 255 - color),
    )
}
