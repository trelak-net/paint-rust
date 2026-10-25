//! Horizontal OpenType shaping, script itemization and Unicode bidirectional order.
use super::fallback::{PreparedFont, covers, ignorable};
use ab_glyph::{Font, Glyph, GlyphId, point};
use std::{
    ops::Range,
    sync::atomic::{AtomicBool, Ordering},
};
use unicode_script::{Script, UnicodeScript};
use unicode_segmentation::UnicodeSegmentation;

pub struct Line {
    pub glyphs: Vec<Glyph>,
    pub width: f32,
    pub font_indices: Vec<usize>,
}

pub fn line(
    face: &rustybuzz::Face<'_>,
    font: &impl Font,
    text: &str,
    size: f32,
    spacing: f32,
    cancel: &AtomicBool,
) -> Option<Line> {
    line_with_fallback(face, font, text, size, spacing, &[], cancel)
}

pub(crate) fn line_with_fallback(
    face: &rustybuzz::Face<'_>,
    font: &impl Font,
    text: &str,
    size: f32,
    spacing: f32,
    fallbacks: &[PreparedFont<'_>],
    cancel: &AtomicBool,
) -> Option<Line> {
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    let text = text.replace('\t', "    ").replace('\r', "");
    let bidi = unicode_bidi::BidiInfo::new(&text, None);
    let mut shaped = Vec::new();
    let em_pixels = size * face.units_per_em() as f32 / font.height_unscaled();

    for paragraph in &bidi.paragraphs {
        let (levels, runs) = bidi.visual_runs(paragraph, paragraph.range.clone());
        for run in runs {
            let rtl = levels[run.start].is_rtl();
            let mut segments = Vec::new();
            for segment in scripts(&text, run) {
                segments.extend(font_segments(&text, segment, face, font, fallbacks));
            }
            if rtl {
                segments.reverse();
            }
            for (segment, font_index) in segments {
                let (face, raster): (&rustybuzz::Face<'_>, &dyn Font) = if font_index == 0 {
                    (face, font)
                } else {
                    let fallback = &fallbacks[font_index - 1];
                    (&fallback.face, &fallback.raster)
                };
                let scale = em_pixels / face.units_per_em() as f32;
                let glyph_size = scale * raster.height_unscaled();
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
                    let ch = text[segment.start + info.cluster as usize..].chars().next();
                    if ch.is_some_and(|ch| ignorable(ch) && !ch.is_whitespace()) {
                        continue;
                    }
                    shaped.push((
                        GlyphId(info.glyph_id as u16),
                        position.x_offset as f32 * scale,
                        -(position.y_offset as f32) * scale,
                        position.x_advance as f32 * scale,
                        segment.start + info.cluster as usize,
                        font_index,
                        !ch.is_some_and(char::is_whitespace),
                        glyph_size,
                    ));
                }
            }
        }
    }
    let mut x = 0.0;
    let mut glyphs = Vec::with_capacity(shaped.len());
    let mut font_indices = Vec::with_capacity(shaped.len());
    for (i, &(id, dx, dy, advance, cluster, font_index, visible, glyph_size)) in
        shaped.iter().enumerate()
    {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        if visible {
            glyphs.push(id.with_scale_and_position(glyph_size, point(x + dx, dy)));
            font_indices.push(font_index);
        }
        x += advance;
        if shaped.get(i + 1).is_some_and(|next| next.4 != cluster) {
            x += spacing;
        }
    }
    Some(Line {
        glyphs,
        width: x,
        font_indices,
    })
}

// Prefer one face for an entire script segment so joining and ligatures remain
// intact. When no face covers it, split only at extended grapheme boundaries,
// then coalesce adjacent ranges using the same face before shaping.
fn font_segments(
    text: &str,
    segment: Range<usize>,
    primary_face: &rustybuzz::Face<'_>,
    primary: &impl Font,
    fallbacks: &[PreparedFont<'_>],
) -> Vec<(Range<usize>, usize)> {
    let choose = |text: &str| {
        if covers(primary_face, primary, text) {
            Some(0)
        } else {
            fallbacks
                .iter()
                .position(|font| covers(&font.face, &font.raster, text))
                .map(|i| i + 1)
        }
    };
    if fallbacks.is_empty() {
        return vec![(segment, 0)];
    }
    if let Some(index) = choose(&text[segment.clone()]) {
        return vec![(segment, index)];
    }
    let mut result: Vec<(Range<usize>, usize)> = Vec::new();
    for (offset, grapheme) in text[segment.clone()].grapheme_indices(true) {
        let index = choose(grapheme).unwrap_or(0);
        let start = segment.start + offset;
        let end = start + grapheme.len();
        if let Some((prior, prior_index)) = result.last_mut()
            && *prior_index == index
        {
            prior.end = end;
        } else {
            result.push((start..end, index));
        }
    }
    result
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

#[cfg(test)]
mod fallback_tests {
    use super::*;
    use ab_glyph::FontArc;

    fn font(name: &str) -> FontArc {
        let definitions = eframe::egui::FontDefinitions::default();
        FontArc::try_from_vec(definitions.font_data[name].font.to_vec()).unwrap()
    }

    #[test]
    fn fallback_keeps_combining_clusters_and_bidirectional_order() {
        let primary = font("NotoEmoji-Regular");
        let fallback = font("Ubuntu-Light");
        let face = rustybuzz::Face::from_slice(primary.font_data(), 0).unwrap();
        let fonts = [PreparedFont {
            face: rustybuzz::Face::from_slice(fallback.font_data(), 0).unwrap(),
            raster: ab_glyph::FontRef::try_from_slice(fallback.font_data()).unwrap(),
        }];
        let cancel = AtomicBool::new(false);
        let shape =
            |text| line_with_fallback(&face, &primary, text, 32.0, 0.0, &fonts, &cancel).unwrap();
        let combining = shape("e\u{301}");
        let composed = shape("é");
        assert_eq!(combining.glyphs, composed.glyphs);
        assert!(!combining.glyphs.is_empty());
        assert!(combining.font_indices.iter().all(|&index| index == 1));
        let rtl = shape("\u{202e}ABC\u{202c}");
        let letters: Vec<_> = rtl
            .glyphs
            .iter()
            .filter(|g| "ABC".chars().any(|ch| fallback.glyph_id(ch) == g.id))
            .map(|g| g.id)
            .collect();
        assert_eq!(
            letters,
            "CBA"
                .chars()
                .map(|ch| fallback.glyph_id(ch))
                .collect::<Vec<_>>()
        );
        assert!(rtl.font_indices.iter().all(|&index| index == 1));
    }

    #[test]
    fn supported_primary_text_and_missing_emoji_use_distinct_faces() {
        let primary = font("Ubuntu-Light");
        let fallback = font("NotoEmoji-Regular");
        let face = rustybuzz::Face::from_slice(primary.font_data(), 0).unwrap();
        let fonts = [PreparedFont {
            face: rustybuzz::Face::from_slice(fallback.font_data(), 0).unwrap(),
            raster: ab_glyph::FontRef::try_from_slice(fallback.font_data()).unwrap(),
        }];
        let cancel = AtomicBool::new(false);
        let mixed =
            line_with_fallback(&face, &primary, "A😀B", 32.0, 2.0, &fonts, &cancel).unwrap();
        assert_eq!(mixed.font_indices, [0, 1, 0]);
        let primary_em =
            mixed.glyphs[0].scale.y * face.units_per_em() as f32 / primary.height_unscaled();
        let fallback_em = mixed.glyphs[1].scale.y * fonts[0].face.units_per_em() as f32
            / fallback.height_unscaled();
        assert!((primary_em - fallback_em).abs() < 0.0001);
        assert!(mixed.glyphs.iter().all(|g| g.id.0 != 0));
        let unchanged =
            line_with_fallback(&face, &primary, "ffi", 32.0, 0.0, &fonts, &cancel).unwrap();
        let baseline = line(&face, &primary, "ffi", 32.0, 0.0, &cancel).unwrap();
        assert_eq!(unchanged.glyphs, baseline.glyphs);
        assert_eq!(unchanged.width, baseline.width);
    }
}
