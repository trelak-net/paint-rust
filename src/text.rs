use crate::document::{Pixel, Selection, blend};
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
    pub alignment: Alignment,
    pub letter_spacing: f32,
    pub line_spacing: f32,
    pub antialias: bool,
}
impl Default for TextLayout {
    fn default() -> Self {
        Self {
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
    let font = style.font.as_scaled(style.size);
    let mut widths = Vec::new();
    for line in style.text.split('\n') {
        let mut advance = 0.0;
        let mut previous = None;
        for ch in line.chars() {
            if cancel.load(Ordering::Relaxed) {
                return false;
            }
            if ch == '\t' {
                advance += font.h_advance(font.glyph_id(' ')) * 4.0;
                previous = None;
            } else if !ch.is_control() {
                let id = font.glyph_id(ch);
                if let Some(prior) = previous {
                    advance += font.kern(prior, id) + style.layout.letter_spacing;
                }
                advance += font.h_advance(id);
                previous = Some(id);
            }
        }
        widths.push(advance);
    }
    let factor = match style.layout.alignment {
        Alignment::Left => 0.0,
        Alignment::Center => 0.5,
        Alignment::Right => 1.0,
    };
    let mut line = 0;
    let mut caret = point(
        style.origin.0 - widths[0] * factor,
        style.origin.1 + font.ascent(),
    );
    let line_height = (font.height() + font.line_gap()) * style.layout.line_spacing;
    let mut previous = None;
    for ch in style.text.chars() {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        if ch == '\n' {
            line += 1;
            caret.x = style.origin.0 - widths[line] * factor;
            caret.y += line_height;
            previous = None;
            continue;
        }
        if ch == '\t' {
            caret.x += font.h_advance(font.glyph_id(' ')) * 4.0;
            previous = None;
            continue;
        }
        if ch.is_control() {
            continue;
        }
        let id = font.glyph_id(ch);
        if let Some(prior) = previous {
            caret.x += font.kern(prior, id) + style.layout.letter_spacing;
        }
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
                    pixels[i] = blend(
                        pixels[i],
                        style.color,
                        if style.layout.antialias {
                            (coverage * 255.0).round().clamp(0.0, 255.0) as u8
                        } else if coverage >= 0.5 {
                            255
                        } else {
                            0
                        },
                    );
                });
            }
        }
        caret.x += font.h_advance(id);
        previous = Some(id);
    }
    !cancel.load(Ordering::Relaxed)
}

/// Load a bounded local font without modifying any editor state.
pub fn load_font_cancellable(
    path: &std::path::Path,
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
    let font =
        FontArc::try_from_vec(data).map_err(|_| "Unsupported or invalid font file.".to_string())?;
    if cancel.load(Ordering::Relaxed) {
        return Err(crate::files::CANCELLED.into());
    }
    Ok(font)
}
