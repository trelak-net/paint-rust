//! Horizontal OpenType shaping, script itemization and Unicode bidirectional order.
use ab_glyph::{Font, FontArc, Glyph, GlyphId, point};
use std::{
    ops::Range,
    sync::atomic::{AtomicBool, Ordering},
};
use unicode_script::{Script, UnicodeScript};

pub struct Line {
    pub glyphs: Vec<Glyph>,
    pub width: f32,
}

pub fn line(
    face: &rustybuzz::Face<'_>,
    font: &FontArc,
    text: &str,
    size: f32,
    spacing: f32,
    cancel: &AtomicBool,
) -> Option<Line> {
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    let text = text.replace('\t', "    ").replace('\r', "");
    let bidi = unicode_bidi::BidiInfo::new(&text, None);
    let mut shaped = Vec::new();
    let scale = size / font.height_unscaled();
    for paragraph in &bidi.paragraphs {
        let (levels, runs) = bidi.visual_runs(paragraph, paragraph.range.clone());
        for run in runs {
            let rtl = levels[run.start].is_rtl();
            let mut segments = scripts(&text, run);
            if rtl {
                segments.reverse();
            }
            for segment in segments {
                if cancel.load(Ordering::Relaxed) {
                    return None;
                }
                let mut buffer = rustybuzz::UnicodeBuffer::new();
                buffer.push_str(&text[segment.clone()]);
                buffer.set_direction(if rtl {
                    rustybuzz::Direction::RightToLeft
                } else {
                    rustybuzz::Direction::LeftToRight
                });
                buffer.guess_segment_properties();
                let features = if spacing != 0.0 {
                    vec!["liga=0".parse().ok()?, "clig=0".parse().ok()?]
                } else {
                    Vec::new()
                };
                let result = rustybuzz::shape(face, &features, buffer);
                for (info, position) in result.glyph_infos().iter().zip(result.glyph_positions()) {
                    shaped.push((
                        GlyphId(info.glyph_id as u16),
                        position.x_offset as f32 * scale,
                        -(position.y_offset as f32) * scale,
                        position.x_advance as f32 * scale,
                        segment.start + info.cluster as usize,
                    ));
                }
            }
        }
    }
    let mut x = 0.0;
    let mut glyphs = Vec::with_capacity(shaped.len());
    for (i, &(id, dx, dy, advance, cluster)) in shaped.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        glyphs.push(id.with_scale_and_position(size, point(x + dx, dy)));
        x += advance;
        if shaped.get(i + 1).is_some_and(|next| next.4 != cluster) {
            x += spacing;
        }
    }
    Some(Line { glyphs, width: x })
}

fn scripts(text: &str, run: Range<usize>) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = run.start;
    let mut current = None;
    for (offset, ch) in text[run.clone()].char_indices() {
        let script = ch.script();
        if matches!(script, Script::Common | Script::Inherited | Script::Unknown) {
            continue;
        }
        if current.is_some_and(|prior| prior != script) {
            ranges.push(start..run.start + offset);
            start = run.start + offset;
        }
        current = Some(script);
    }
    if start < run.end {
        ranges.push(start..run.end);
    }
    ranges
}
