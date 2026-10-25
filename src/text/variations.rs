//! Bounded variable-font coordinates shared by shaping and outline rasterization.
use ab_glyph::{Font, FontArc, FontRef, VariableFont};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coordinate {
    pub tag: [u8; 4],
    pub value: f32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Settings([Option<Coordinate>; 16]);
impl Settings {
    pub fn get(&self, tag: [u8; 4]) -> Option<f32> {
        self.0
            .iter()
            .flatten()
            .find(|c| c.tag == tag)
            .map(|c| c.value)
    }
    pub fn set(&mut self, tag: [u8; 4], value: f32) -> bool {
        if !value.is_finite() {
            return false;
        }
        let slot = self
            .0
            .iter()
            .position(|c| c.is_some_and(|c| c.tag == tag))
            .or_else(|| self.0.iter().position(Option::is_none));
        if let Some(index) = slot {
            self.0[index] = Some(Coordinate { tag, value });
            true
        } else {
            false
        }
    }
    pub(super) fn apply(&self, font: &mut FontRef<'_>, face: &mut rustybuzz::Face<'_>) -> bool {
        let axes = font.variations();
        let mut coordinates = Vec::new();
        for c in self.0.iter().flatten() {
            let Some(axis) = axes.iter().find(|axis| axis.tag == c.tag) else {
                return false;
            };
            if !c.value.is_finite() || !(axis.min_value..=axis.max_value).contains(&c.value) {
                return false;
            }
            if !font.set_variation(&c.tag, c.value) {
                return false;
            }
            coordinates.push(rustybuzz::Variation {
                tag: rustybuzz::ttf_parser::Tag::from_bytes(&c.tag),
                value: c.value,
            });
        }
        face.set_variations(&coordinates);
        true
    }
}

pub fn axes(font: &FontArc, index: u32) -> Vec<ab_glyph::VariationAxis> {
    FontRef::try_from_slice_and_index(font.font_data(), index)
        .map(|font| {
            font.variations()
                .into_iter()
                .filter(|a| !a.hidden)
                .take(16)
                .collect()
        })
        .unwrap_or_default()
}
