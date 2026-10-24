use crate::document::{Pixel, Selection, blend};
use ab_glyph::{Font, FontArc, ScaleFont, point};

pub struct TextStyle<'a> {
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
    let font = style.font.as_scaled(style.size);
    let mut caret = point(style.origin.0, style.origin.1 + font.ascent());
    let line_height = font.height() + font.line_gap();
    let mut previous = None;
    for ch in style.text.chars() {
        if ch == '\n' {
            caret.x = style.origin.0;
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
            caret.x += font.kern(prior, id);
        }
        if let Some(glyph) = font.outline_glyph(id.with_scale_and_position(style.size, caret)) {
            let bounds = glyph.px_bounds();
            if bounds.max.x > 0.0
                && bounds.max.y > 0.0
                && bounds.min.x < width as f32
                && bounds.min.y < height as f32
            {
                glyph.draw(|x, y, coverage| {
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
                        (coverage * 255.0).round().clamp(0.0, 255.0) as u8,
                    );
                });
            }
        }
        caret.x += font.h_advance(id);
        previous = Some(id);
    }
}
