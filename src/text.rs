pub mod shaping;
pub mod variations;
use crate::document::{Pixel, Selection, blend_layer};
use ab_glyph::{Font, FontArc, ScaleFont, point};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Center,
    Right,
}
impl Alignment {
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Center => "Center",
            Self::Right => "Right",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextLayout {
    pub blend_mode: crate::document::BlendMode,
    pub font_index: u32,
    pub font_axes: variations::Settings,
    pub underline: bool,
    pub strikethrough: bool,
    pub alignment: Alignment,
    pub letter_spacing: f32,
    pub line_spacing: f32,
    pub antialias: bool,
}
impl Default for TextLayout {
    fn default() -> Self {
        Self {
            blend_mode: Default::default(),
            font_index: 0,
            font_axes: Default::default(),
            underline: false,
            strikethrough: false,
            alignment: Alignment::Left,
            letter_spacing: 0.0,
            line_spacing: 1.0,
            antialias: true,
        }
    }
}

pub struct TextStyle<'a> {
    pub layout: TextLayout,
    pub font: &'a FontArc,
    pub text: &'a str,
    pub origin: (f32, f32),
    pub size: f32,
    pub color: Pixel,
}

pub fn render(
    pixels: &mut [Pixel],
    width: u32,
    height: u32,
    selection: Option<Selection>,
    style: TextStyle<'_>,
) {
    render_cancellable(
        pixels,
        width,
        height,
        selection,
        style,
        &AtomicBool::new(false),
    );
}

pub fn render_cancellable(
    pixels: &mut [Pixel],
    width: u32,
    height: u32,
    selection: Option<Selection>,
    style: TextStyle<'_>,
    cancel: &AtomicBool,
) -> bool {
    if cancel.load(Ordering::Relaxed)
        || !style.size.is_finite()
        || !(1.0..=512.0).contains(&style.size)
        || style.text.len() > 16384
        || !style.layout.letter_spacing.is_finite()
        || !(-32.0..=128.0).contains(&style.layout.letter_spacing)
        || !style.layout.line_spacing.is_finite()
        || !(0.5..=3.0).contains(&style.layout.line_spacing)
        || !style.origin.0.is_finite()
        || !style.origin.1.is_finite()
        || width == 0
        || height == 0
        || u64::from(width) * u64::from(height) != pixels.len() as u64
    {
        return false;
    }
    let Ok(mut raster_font) = ab_glyph::FontRef::try_from_slice_and_index(
        style.font.font_data(),
        style.layout.font_index,
    ) else {
        return false;
    };
    let Some(mut face) =
        rustybuzz::Face::from_slice(style.font.font_data(), style.layout.font_index)
    else {
        return false;
    };
    if !style.layout.font_axes.apply(&mut raster_font, &mut face) {
        return false;
    }
    let font = raster_font.as_scaled(style.size);
    let mut lines = Vec::new();
    for line in style.text.split('\n') {
        let Some(line) = shaping::line(
            &face,
            &raster_font,
            line,
            style.size,
            style.layout.letter_spacing,
            cancel,
        ) else {
            return false;
        };
        lines.push(line);
    }
    let factor = match style.layout.alignment {
        Alignment::Left => 0.0,
        Alignment::Center => 0.5,
        Alignment::Right => 1.0,
    };
    let line_height = (font.height() + font.line_gap()) * style.layout.line_spacing;
    for (line_index, line) in lines.iter().enumerate() {
        let origin = point(
            style.origin.0 - line.width * factor,
            style.origin.1 + font.ascent() + line_index as f32 * line_height,
        );
        for shaped in &line.glyphs {
            if cancel.load(Ordering::Relaxed) {
                return false;
            }
            let caret = point(origin.x + shaped.position.x, origin.y + shaped.position.y);
            let id = shaped.id;
            if let Some(glyph) = font.outline_glyph(id.with_scale_and_position(style.size, caret)) {
                let bounds = glyph.px_bounds();
                if bounds.max.x > 0.0
                    && bounds.max.y > 0.0
                    && bounds.min.x < width as f32
                    && bounds.min.y < height as f32
                {
                    glyph.draw(|x, y, coverage| {
                        if cancel.load(Ordering::Relaxed) {
                            return;
                        }
                        let x = bounds.min.x as i64 + i64::from(x);
                        let y = bounds.min.y as i64 + i64::from(y);
                        if x < 0 || y < 0 || x >= i64::from(width) || y >= i64::from(height) {
                            return;
                        }
                        if selection
                            .as_ref()
                            .is_some_and(|r| !r.contains(x as usize, y as usize))
                        {
                            return;
                        }
                        let i = y as usize * width as usize + x as usize;
                        pixels[i] = blend_layer(
                            pixels[i],
                            style.color,
                            if style.layout.antialias {
                                (coverage * 255.0).round().clamp(0.0, 255.0) as u8
                            } else if coverage >= 0.5 {
                                255
                            } else {
                                0
                            },
                            style.layout.blend_mode,
                        );
                    });
                }
            }
        }
    }
    if style.layout.underline || style.layout.strikethrough {
        let scale = style.size / raster_font.height_unscaled();
        let underline = face
            .underline_metrics()
            .map(|m| {
                (
                    -(m.position as f32) * scale,
                    (m.thickness as f32 * scale).max(1.0),
                )
            })
            .unwrap_or((style.size * 0.08, (style.size / 16.0).max(1.0)));
        let strike = face
            .strikeout_metrics()
            .map(|m| {
                (
                    -(m.position as f32) * scale,
                    (m.thickness as f32 * scale).max(1.0),
                )
            })
            .unwrap_or((-style.size * 0.3, (style.size / 16.0).max(1.0)));
        for (line, shaped) in lines.iter().enumerate() {
            let advance = shaped.width;
            if cancel.load(Ordering::Relaxed) {
                return false;
            }
            if advance <= 0.0 {
                continue;
            }
            let left = style.origin.0 - advance * factor;
            let right = left + advance;
            let baseline = style.origin.1 + font.ascent() + line as f32 * line_height;
            for (enabled, (offset, thickness)) in [
                (style.layout.underline, underline),
                (style.layout.strikethrough, strike),
            ] {
                let center = baseline + offset;
                if !enabled {
                    continue;
                }
                let top = center - thickness * 0.5;
                let bottom = center + thickness * 0.5;
                for y in (top.floor().max(0.0) as u32).min(height)
                    ..(bottom.ceil().max(0.0) as u32).min(height)
                {
                    if cancel.load(Ordering::Relaxed) {
                        return false;
                    }
                    let vertical = (bottom.min(y as f32 + 1.0) - top.max(y as f32)).clamp(0.0, 1.0);
                    for x in (left.floor().max(0.0) as u32).min(width)
                        ..(right.ceil().max(0.0) as u32).min(width)
                    {
                        if selection
                            .as_ref()
                            .is_some_and(|s| !s.contains(x as usize, y as usize))
                        {
                            continue;
                        }
                        let horizontal =
                            (right.min(x as f32 + 1.0) - left.max(x as f32)).clamp(0.0, 1.0);
                        let coverage = horizontal * vertical;
                        let opacity = if style.layout.antialias {
                            (coverage * 255.0).round() as u8
                        } else if coverage >= 0.5 {
                            255
                        } else {
                            0
                        };
                        let index = (y * width + x) as usize;
                        pixels[index] = blend_layer(
                            pixels[index],
                            style.color,
                            opacity,
                            style.layout.blend_mode,
                        );
                    }
                }
            }
        }
    }
    !cancel.load(Ordering::Relaxed)
}

/// Load a bounded local font without modifying any editor state.
pub fn load_font_cancellable(
    path: &std::path::Path,
    cancel: &AtomicBool,
) -> Result<FontArc, String> {
    load_font_face_cancellable(path, 0, cancel)
}

pub fn load_font_face_cancellable(
    path: &std::path::Path,
    index: u32,
    cancel: &AtomicBool,
) -> Result<FontArc, String> {
    use std::io::Read;
    const LIMIT: usize = 16 * 1024 * 1024;
    if cancel.load(Ordering::Relaxed) {
        return Err(crate::files::CANCELLED.into());
    }
    let mut file =
        std::fs::File::open(path).map_err(|error| format!("Could not open font: {error}"))?;
    if file.metadata().map_err(|error| error.to_string())?.len() > LIMIT as u64 {
        return Err("Font files are limited to 16 MiB.".into());
    }
    let mut data = Vec::new();
    let mut buffer = [0_u8; 65536];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(crate::files::CANCELLED.into());
        }
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("Could not read font: {error}"))?;
        if count == 0 {
            break;
        }
        if data.len() + count > LIMIT {
            return Err("Font files are limited to 16 MiB.".into());
        }
        data.extend_from_slice(&buffer[..count]);
    }
    let font = FontArc::new(
        ab_glyph::FontVec::try_from_vec_and_index(data, index)
            .map_err(|_| "Unsupported or invalid font face.".to_string())?,
    );
    if cancel.load(Ordering::Relaxed) {
        return Err(crate::files::CANCELLED.into());
    }
    Ok(font)
}
