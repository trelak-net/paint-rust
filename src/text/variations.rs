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
    pub(super) fn overlay_supported(&mut self, requested: Self, font: &FontArc, index: u32) {
        let Ok(face) = FontRef::try_from_slice_and_index(font.font_data(), index) else {
            return;
        };
        for axis in face.variations() {
            // Registered design axes have common semantics across families.
            if !matches!(&axis.tag, b"wght" | b"wdth" | b"ital" | b"slnt" | b"opsz") {
                continue;
            }
            if let Some(value) = requested.get(axis.tag) {
                self.set(axis.tag, value.clamp(axis.min_value, axis.max_value));
            }
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

/// Decode one-based named-instance coordinates without trusting table offsets.
/// Layout: https://learn.microsoft.com/en-us/typography/opentype/spec/fvar
pub(crate) fn named_instance(font: &FontArc, index: u32, instance: u16) -> Option<Settings> {
    if instance == 0 {
        return Some(Settings::default());
    }
    let face = rustybuzz::Face::from_slice(font.font_data(), index)?;
    let table = face
        .raw_face()
        .table(rustybuzz::ttf_parser::Tag::from_bytes(b"fvar"))?;
    instance_coordinates(table, instance)
}
fn instance_coordinates(data: &[u8], instance: u16) -> Option<Settings> {
    let u16_at = |offset| {
        Some(u16::from_be_bytes(
            data.get(offset..offset + 2)?.try_into().ok()?,
        ))
    };
    if u16_at(0)? != 1 || u16_at(2)? != 0 {
        return None;
    }
    let offset = usize::from(u16_at(4)?);
    let axes = usize::from(u16_at(8)?);
    let axis_size = usize::from(u16_at(10)?);
    let count = u16_at(12)?;
    let record_size = usize::from(u16_at(14)?);
    if offset < 16
        || !(1..=16).contains(&axes)
        || axis_size < 20
        || instance == 0
        || instance > count
        || record_size < 4 + axes * 4
    {
        return None;
    }
    let record = offset
        .checked_add(axes.checked_mul(axis_size)?)?
        .checked_add(usize::from(instance - 1).checked_mul(record_size)?)?;
    data.get(record..record.checked_add(record_size)?)?;
    let fixed = |offset| {
        Some(i32::from_be_bytes(data.get(offset..offset + 4)?.try_into().ok()?) as f32 / 65536.0)
    };
    let mut result = Settings::default();
    for axis in 0..axes {
        let start = offset + axis * axis_size;
        let tag: [u8; 4] = data.get(start..start + 4)?.try_into().ok()?;
        if result.get(tag).is_some() {
            return None;
        }
        let (min, default, max) = (fixed(start + 4)?, fixed(start + 8)?, fixed(start + 12)?);
        if min > default || default > max {
            return None;
        }
        let value = fixed(record + 4 + axis * 4)?.clamp(min, max);
        if !result.set(tag, value) {
            return None;
        }
    }
    Some(result)
}

#[cfg(test)]
mod instance_tests {
    use super::*;
    fn table() -> Vec<u8> {
        let mut data = vec![0; 52];
        for (offset, value) in [
            (0, 1_u16),
            (4, 16),
            (6, 2),
            (8, 1),
            (10, 20),
            (12, 2),
            (14, 8),
        ] {
            data[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        }
        data[16..20].copy_from_slice(b"wght");
        for (offset, value) in [(20, 100_i32), (24, 400), (28, 900), (40, 400), (48, 700)] {
            data[offset..offset + 4].copy_from_slice(&(value * 65536).to_be_bytes());
        }
        data
    }
    #[test]
    fn named_instances_read_coordinates_and_reject_truncated_or_invalid_tables() {
        let data = table();
        assert_eq!(
            instance_coordinates(&data, 1).unwrap().get(*b"wght"),
            Some(400.0)
        );
        assert_eq!(
            instance_coordinates(&data, 2).unwrap().get(*b"wght"),
            Some(700.0)
        );
        assert!(instance_coordinates(&data, 0).is_none());
        assert!(instance_coordinates(&data, 3).is_none());
        for end in 0..data.len() {
            assert!(instance_coordinates(&data[..end], 2).is_none());
        }
        for (offset, value) in [(4, 0_u16), (8, 17), (10, 19), (14, 7)] {
            let mut malformed = data.clone();
            malformed[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
            assert!(instance_coordinates(&malformed, 2).is_none());
        }
    }
}
