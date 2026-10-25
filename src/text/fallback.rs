//! Per-preview, bounded outline-font fallback. No font discovery runs on the UI thread.
use ab_glyph::{Font, FontArc, FontRef};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone)]
pub struct FontFace {
    pub font: FontArc,
    pub index: u32,
}

pub(crate) struct PreparedFont<'a> {
    pub face: rustybuzz::Face<'a>,
    pub raster: FontRef<'a>,
}

pub(crate) fn ignorable(ch: char) -> bool {
    ch.is_control()
        || ch.is_whitespace()
        || matches!(ch as u32,
        0x00ad | 0x034f | 0x061c | 0x180b..=0x180f | 0x200b..=0x200f |
        0x202a..=0x202e | 0x2060..=0x206f | 0xfe00..=0xfe0f | 0xe0100..=0xe01ef)
}

pub(crate) fn supports(font: &dyn Font, ch: char) -> bool {
    if ignorable(ch) {
        return true;
    }
    let id = font.glyph_id(ch);
    id.0 != 0 && font.outline(id).is_some()
}

// Shaping can compose a base and mark into a glyph even when the mark has no
// standalone cmap entry. Test the shaped cluster before declaring it missing.
pub(crate) fn covers(face: &rustybuzz::Face<'_>, font: &dyn Font, text: &str) -> bool {
    if text.chars().all(|ch| supports(font, ch)) {
        return true;
    }
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    let shaped = rustybuzz::shape(face, &[], buffer);
    !shaped.glyph_infos().is_empty()
        && shaped.glyph_infos().iter().all(|info| {
            let invisible = text
                .get(info.cluster as usize..)
                .and_then(|s| s.chars().next())
                .is_some_and(ignorable);
            invisible
                || (info.glyph_id != 0
                    && font
                        .outline(ab_glyph::GlyphId(info.glyph_id as u16))
                        .is_some())
        })
}

impl FontFace {
    fn covers(&self, text: &str) -> bool {
        rustybuzz::Face::from_slice(self.font.font_data(), self.index)
            .is_some_and(|face| covers(&face, &self.font, text))
    }
}

#[derive(Default)]
pub struct Resolver {
    family: Option<String>,
    candidates: Vec<crate::fonts::InstalledFont>,
    examined: HashSet<(PathBuf, u32)>,
    searched: HashSet<String>,
    fonts: Vec<FontFace>,
    bytes: usize,
}
impl Resolver {
    pub fn fonts(&self) -> &[FontFace] {
        &self.fonts
    }

    /// Missing Fontconfig or unavailable glyphs keep the primary font's missing
    /// glyph. Cancellation alone interrupts the preview; failures remain retryable
    /// when the user chooses a different font or opens a new text operation.
    pub fn prepare(
        &mut self,
        primary: &FontArc,
        index: u32,
        text: &str,
        cancel: &AtomicBool,
    ) -> bool {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        if text.len() > 16384 {
            return false;
        }
        let Ok(primary_raster) = FontRef::try_from_slice_and_index(primary.font_data(), index)
        else {
            return false;
        };
        let Some(face) = rustybuzz::Face::from_slice(primary.font_data(), index) else {
            return false;
        };
        let missing: HashSet<String> = text
            .graphemes(true)
            .filter(|grapheme| !covers(&face, &primary_raster, grapheme))
            .map(str::to_owned)
            .collect();
        if missing.is_empty() {
            return true;
        }

        let family = face
            .names()
            .into_iter()
            .find_map(|name| {
                (name.name_id == rustybuzz::ttf_parser::name_id::FAMILY)
                    .then(|| name.to_string())
                    .flatten()
            })
            .unwrap_or_else(|| "sans-serif".into());
        if self.family.as_ref() != Some(&family) {
            let candidates = crate::fonts::fallback_candidates(&family, cancel);
            if cancel.load(Ordering::Relaxed) {
                return false;
            }
            self.family = Some(family);
            self.candidates = candidates.unwrap_or_default();
            self.candidates.truncate(128);
            self.examined.clear();
            self.searched.clear();
            self.fonts.clear();
            self.bytes = 0;
        }
        let mut missing: HashSet<_> = missing
            .into_iter()
            .filter(|grapheme| !self.fonts.iter().any(|font| font.covers(grapheme)))
            .collect();
        if missing != self.searched {
            self.examined.clear();
            self.searched = missing.clone();
        }
        let mut scanned_bytes = 0_u64;
        const BUDGET: usize = 64 * 1024 * 1024;
        for candidate in &self.candidates {
            if cancel.load(Ordering::Relaxed) {
                return false;
            }
            if missing.is_empty() || self.fonts.len() >= 32 {
                break;
            }
            if !self
                .examined
                .insert((candidate.path.clone(), candidate.index))
            {
                continue;
            }
            let Ok(metadata) = candidate.path.metadata() else {
                continue;
            };
            if scanned_bytes + metadata.len() > 128 * 1024 * 1024 {
                self.examined
                    .remove(&(candidate.path.clone(), candidate.index));
                break;
            }
            scanned_bytes += metadata.len();
            if metadata.len() > (BUDGET - self.bytes) as u64 {
                continue;
            }
            let Ok(font) =
                super::load_font_face_cancellable(&candidate.path, candidate.index, cancel)
            else {
                if cancel.load(Ordering::Relaxed) {
                    self.examined
                        .remove(&(candidate.path.clone(), candidate.index));
                    return false;
                }
                continue;
            };
            let Some(face) = rustybuzz::Face::from_slice(font.font_data(), candidate.index) else {
                continue;
            };
            if !missing
                .iter()
                .any(|grapheme| covers(&face, &font, grapheme))
            {
                continue;
            }
            if font.font_data().len() > BUDGET - self.bytes {
                break;
            }
            self.bytes += font.font_data().len();
            missing.retain(|grapheme| !covers(&face, &font, grapheme));
            self.fonts.push(FontFace {
                font,
                index: candidate.index,
            });
        }
        !cancel.load(Ordering::Relaxed)
    }
}
