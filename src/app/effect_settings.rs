use super::*;
use std::io::{Read, Write};

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
pub(super) struct Settings {
    version: u32,
    pub last: Option<Effect>,
    pub parameters: Vec<Effect>,
}
impl Settings {
    pub fn new(last: Option<Effect>, mut parameters: Vec<Effect>) -> Self {
        parameters.sort_by_key(|effect| effect.name());
        Self {
            version: 1,
            last,
            parameters,
        }
    }
    fn path() -> Option<PathBuf> {
        panels::Layout::path().map(|path| path.with_file_name("effects.json"))
    }
    fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.parameters.len() > 64
            || self.last.is_some_and(|effect| !effect.valid_settings())
            || self
                .parameters
                .iter()
                .any(|effect| !effect.valid_settings())
        {
            return Err("Unsupported or invalid effect settings".into());
        }
        let mut kinds = std::collections::HashSet::new();
        if self
            .parameters
            .iter()
            .any(|effect| !kinds.insert(std::mem::discriminant(effect)))
        {
            return Err("Duplicate effect settings".into());
        }
        Ok(())
    }
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 65536 {
            return Err("Effect settings exceed 64 KiB".into());
        }
        let settings: Self = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        settings.validate()?;
        Ok(settings)
    }
    pub fn load() -> Result<Self, String> {
        let Some(path) = Self::path() else {
            return Ok(Self::new(None, Vec::new()));
        };
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::new(None, Vec::new()));
            }
            Err(error) => return Err(error.to_string()),
        };
        let mut bytes = Vec::new();
        file.take(65537)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        Self::decode(&bytes)
    }
    pub fn save(&self) -> Result<(), String> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        Self::decode(&bytes)?;
        let path = Self::path().ok_or("No configuration directory")?;
        let parent = path.parent().ok_or("No configuration directory")?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let mut file =
            tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.as_file()
            .sync_all()
            .map_err(|error| error.to_string())?;
        file.persist(path).map_err(|error| error.to_string())?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_preserves_curves_levels_seeds_and_last_effect() {
        let mut curves = paint_rust::curves::Curves::default();
        curves.channels[1].insert(100, 170);
        let mut levels = paint_rust::levels::Levels::default();
        levels.channels[2].gamma = 2.2;
        let last = Effect::AddNoise {
            amount: 30,
            coverage: 70,
            monochrome: false,
            seed: 12345,
        };
        let settings = Settings::new(
            Some(last),
            vec![
                last,
                Effect::Curves(curves),
                Effect::Levels(levels),
                Effect::Twist(-123.0),
            ],
        );
        assert_eq!(
            Settings::decode(&serde_json::to_vec(&settings).unwrap()).unwrap(),
            settings
        );
    }
    #[test]
    fn malformed_or_out_of_range_settings_are_rejected() {
        for parameters in [
            vec![Effect::Posterize(0)],
            vec![Effect::GaussianBlur(999)],
            vec![Effect::RedEye(0.0)],
            vec![Effect::Grayscale, Effect::Grayscale],
        ] {
            let settings = Settings::new(None, parameters);
            assert!(Settings::decode(&serde_json::to_vec(&settings).unwrap()).is_err());
        }
        let mut levels = paint_rust::levels::Levels::default();
        levels.channels[0].input_white = 0;
        assert!(
            Settings::new(None, vec![Effect::Levels(levels)])
                .validate()
                .is_err()
        );
        let mut curve = serde_json::to_value(paint_rust::curves::Curve::default()).unwrap();
        for len in [0, 1, 17, 999999] {
            curve["len"] = len.into();
            assert!(serde_json::from_value::<paint_rust::curves::Curve>(curve.clone()).is_err());
        }
        curve["len"] = 2.into();
        curve["points"][1][0] = 0.into();
        assert!(serde_json::from_value::<paint_rust::curves::Curve>(curve).is_err());
        assert!(Settings::decode(&vec![b' '; 65537]).is_err());
        assert!(
            Settings::new(None, vec![Effect::FreeTransform(Default::default())])
                .validate()
                .is_err()
        );
    }
}
