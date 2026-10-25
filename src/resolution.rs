//! Physical resolution represented by standard EXIF rational metadata entries.
use base64::{Engine, engine::general_purpose::STANDARD};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dpi {
    pub x: f64,
    pub y: f64,
}
impl Default for Dpi {
    fn default() -> Self {
        Self { x: 96.0, y: 96.0 }
    }
}
impl Dpi {
    pub fn valid(self) -> bool {
        [self.x, self.y]
            .iter()
            .all(|v| v.is_finite() && (1.0..=10000.0).contains(v))
    }
}
fn attribute<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let (_, tail) = text.split_once(&format!(" {name}=\""))?;
    tail.split_once('"').map(|(value, _)| value)
}
fn tag(value: &str) -> Option<u16> {
    if value.len() > 512 || !value.starts_with("<exif ") || !value.ends_with("/>") {
        return None;
    }
    attribute(value, "id")?.parse().ok()
}
fn payload(value: &str, kind: &str, length: usize) -> Option<Vec<u8>> {
    if attribute(value, "type")? != kind
        || attribute(value, "len")?.parse::<usize>().ok()? != length
    {
        return None;
    }
    let bytes = STANDARD.decode(attribute(value, "value")?).ok()?;
    (bytes.len() == length).then_some(bytes)
}
pub fn read(metadata: &BTreeMap<String, String>) -> Dpi {
    let mut values = [None, None];
    let mut unit = 2;
    for (key, value) in metadata {
        if !key.starts_with("$exif.") {
            continue;
        }
        match tag(value) {
            Some(id @ (282 | 283)) => {
                if let Some(bytes) = payload(value, "5", 8) {
                    let n = u32::from_le_bytes(bytes[..4].try_into().unwrap());
                    let d = u32::from_le_bytes(bytes[4..].try_into().unwrap());
                    if d != 0 {
                        values[(id - 282) as usize] = Some(f64::from(n) / f64::from(d));
                    }
                }
            }
            Some(296) => {
                if let Some(bytes) = payload(value, "3", 2) {
                    unit = u16::from_le_bytes(bytes.try_into().unwrap());
                }
            }
            _ => {}
        }
    }
    let factor = if unit == 3 {
        2.54
    } else if unit == 2 {
        1.0
    } else {
        return Dpi::default();
    };
    let mut dpi = Dpi::default();
    if let Some(x) = values[0] {
        dpi.x = x * factor;
    }
    if let Some(y) = values[1] {
        dpi.y = y * factor;
    }
    if dpi.valid() { dpi } else { Dpi::default() }
}
pub fn write(metadata: &mut BTreeMap<String, String>, dpi: Dpi) -> Result<(), String> {
    if !dpi.valid() {
        return Err("Resolution must be between 1 and 10000 pixels per inch".into());
    }
    let mut next = metadata.clone();
    next.retain(|key, value| {
        !(key.starts_with("$exif.") && matches!(tag(value), Some(282 | 283 | 296)))
    });
    for (id, kind, bytes) in [
        (296, 3, 2u16.to_le_bytes().to_vec()),
        (282, 5, rational(dpi.x)),
        (283, 5, rational(dpi.y)),
    ] {
        let index = (0..4096)
            .find(|i| !next.contains_key(&format!("$exif.tag{i}[0]")))
            .ok_or("Metadata entry limit exceeded")?;
        next.insert(
            format!("$exif.tag{index}[0]"),
            format!(
                "<exif id=\"{id}\" len=\"{}\" type=\"{kind}\" value=\"{}\" />",
                bytes.len(),
                STANDARD.encode(bytes)
            ),
        );
    }
    if next.len() > 4096 {
        return Err("Metadata entry limit exceeded".into());
    }
    *metadata = next;
    Ok(())
}
fn rational(value: f64) -> Vec<u8> {
    let numerator = (value * 10000.0).round() as u32;
    let mut a = numerator;
    let mut b = 10000;
    while b != 0 {
        (a, b) = (b, a % b);
    }
    [(numerator / a).to_le_bytes(), (10000 / a).to_le_bytes()].concat()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Unit {
    #[default]
    Pixels,
    Inches,
    Centimeters,
}
impl Unit {
    pub const ALL: [Self; 3] = [Self::Pixels, Self::Inches, Self::Centimeters];
    pub fn label(self) -> &'static str {
        match self {
            Self::Pixels => "Pixels",
            Self::Inches => "Inches",
            Self::Centimeters => "Centimeters",
        }
    }
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Pixels => "px",
            Self::Inches => "in",
            Self::Centimeters => "cm",
        }
    }
    pub fn pixels_per_unit(self, dpi: f64) -> f64 {
        match self {
            Self::Pixels => 1.0,
            Self::Inches => dpi,
            Self::Centimeters => dpi / 2.54,
        }
    }
}
