use super::*;
use std::io::{Read, Write};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Theme {
    #[default]
    System,
    Light,
    Dark,
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Preferences {
    pub theme: Theme,
    pub pixel_grid: bool,
    pub rulers: bool,
    pub brush_size: f32,
    pub brush: paint_rust::brush::Options,
    pub recent: Vec<PathBuf>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            pixel_grid: false,
            rulers: false,
            brush_size: 12.0,
            brush: Default::default(),
            recent: Vec::new(),
        }
    }
}
impl Preferences {
    fn path() -> Option<PathBuf> {
        panels::Layout::path().map(|p| p.with_file_name("preferences.txt"))
    }
    fn encode(&self) -> String {
        let mut text = format!(
            "preferences-v1\n{} {} {} {} {} {} {} {}\n",
            match self.theme {
                Theme::System => "system",
                Theme::Light => "light",
                Theme::Dark => "dark",
            },
            u8::from(self.pixel_grid),
            self.brush_size,
            self.brush.hardness,
            self.brush.opacity,
            u8::from(self.brush.antialias),
            u8::from(self.brush.blend),
            u8::from(self.rulers)
        );
        for path in self.recent.iter().take(16) {
            if let Some(path) = path.to_str()
                && path.len() <= 8192
            {
                for byte in path.bytes() {
                    text.push_str(&format!("{byte:02x}"));
                }
                text.push('\n');
            }
        }
        text
    }
    fn decode(text: &str) -> Result<Self, String> {
        let invalid = || "Invalid preferences file".to_string();
        if text.len() > 270000 {
            return Err(invalid());
        }
        let mut lines = text.lines();
        if lines.next() != Some("preferences-v1") {
            return Err(invalid());
        }
        let fields: Vec<_> = lines
            .next()
            .ok_or_else(invalid)?
            .split_whitespace()
            .collect();
        if fields.len() != 7 && fields.len() != 8 {
            return Err(invalid());
        }
        let boolean = |v: &str| match v {
            "0" => Ok(false),
            "1" => Ok(true),
            _ => Err(invalid()),
        };
        let number = |v: &str, min: f32, max: f32| -> Result<f32, String> {
            let value = v.parse::<f32>().map_err(|_| invalid())?;
            if value.is_finite() && (min..=max).contains(&value) {
                Ok(value)
            } else {
                Err(invalid())
            }
        };
        let mut result = Self {
            theme: match fields[0] {
                "system" => Theme::System,
                "light" => Theme::Light,
                "dark" => Theme::Dark,
                _ => return Err(invalid()),
            },
            pixel_grid: boolean(fields[1])?,
            rulers: if fields.len() == 8 {
                boolean(fields[7])?
            } else {
                false
            },
            brush_size: number(fields[2], 1.0, 128.0)?,
            brush: paint_rust::brush::Options {
                hardness: number(fields[3], 0.0, 1.0)?,
                opacity: number(fields[4], 0.0, 1.0)?,
                antialias: boolean(fields[5])?,
                blend: boolean(fields[6])?,
            },
            recent: Vec::new(),
        };
        for line in lines {
            if line.is_empty() {
                continue;
            }
            if result.recent.len() >= 16
                || line.len() > 16384
                || !line.len().is_multiple_of(2)
                || !line.is_ascii()
            {
                return Err(invalid());
            }
            let bytes: Result<Vec<_>, _> = (0..line.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&line[i..i + 2], 16))
                .collect();
            let path = PathBuf::from(
                String::from_utf8(bytes.map_err(|_| invalid())?).map_err(|_| invalid())?,
            );
            if !path.is_absolute() {
                return Err(invalid());
            }
            if !result.recent.contains(&path) {
                result.recent.push(path);
            }
        }
        Ok(result)
    }
    pub fn load() -> Result<Self, String> {
        let Some(path) = Self::path() else {
            return Ok(Self::default());
        };
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.to_string()),
        };
        let mut text = String::new();
        file.take(270001)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        Self::decode(&text)
    }
    pub fn save(&self) -> Result<(), String> {
        let path = Self::path().ok_or("No configuration directory")?;
        let parent = path.parent().ok_or("No configuration directory")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        let text = self.encode();
        Self::decode(&text)?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn remember(&mut self, path: &std::path::Path) {
        let Ok(path) = std::path::absolute(path) else {
            return;
        };
        if !path.to_str().is_some_and(|p| p.len() <= 8192) {
            return;
        }
        self.recent.retain(|old| old != &path);
        self.recent.insert(0, path);
        self.recent.truncate(16);
    }
}
impl Editor {
    pub(super) fn apply_theme(&self, ctx: &egui::Context) {
        let theme = match self.preferences.theme {
            Theme::System => self.system_theme,
            Theme::Light => Some(egui::Theme::Light),
            Theme::Dark => Some(egui::Theme::Dark),
        };
        if let Some(theme) = theme {
            ctx.set_theme(theme);
            ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(match theme {
                egui::Theme::Light => egui::SystemTheme::Light,
                egui::Theme::Dark => egui::SystemTheme::Dark,
            }));
        } else {
            ctx.set_theme(egui::ThemePreference::System);
        }
    }
    pub(super) fn preferences_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Theme", |ui| {
            let before = self.preferences.theme;
            for (choice, label) in [
                (Theme::System, "System"),
                (Theme::Light, "Light"),
                (Theme::Dark, "Dark"),
            ] {
                ui.radio_value(&mut self.preferences.theme, choice, label);
            }
            if before != self.preferences.theme {
                self.apply_theme(ui.ctx());
            }
        });
    }
    pub(super) fn recent_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Recent", |ui| {
            if self.preferences.recent.is_empty() {
                ui.label("No recent files");
            }
            for path in self.preferences.recent.clone() {
                let label = path
                    .file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy();
                if ui
                    .button(label)
                    .on_hover_text(path.display().to_string())
                    .clicked()
                {
                    self.open_queue.push_back(path);
                    ui.close();
                }
            }
            ui.separator();
            if ui
                .add_enabled(
                    !self.preferences.recent.is_empty(),
                    egui::Button::new("Clear recent files"),
                )
                .clicked()
            {
                self.preferences.recent.clear();
                ui.close();
            }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_theme_wins_over_desktop_notifications() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.preferences.theme = Theme::Dark;
        editor.apply_theme(&ctx);
        let (send, receive) = std::sync::mpsc::channel();
        editor.appearance = Some(receive);
        send.send(egui::Theme::Light).unwrap();
        let _ = ctx.run(egui::RawInput::default(), |ctx| editor.show(ctx));
        assert_eq!(ctx.theme(), egui::Theme::Dark);
        editor.preferences.theme = Theme::System;
        editor.apply_theme(&ctx);
        assert_eq!(ctx.theme(), egui::Theme::Light);
    }

    #[test]
    fn preferences_roundtrip_bound_recent_paths_and_validate_numbers() {
        let mut settings = Preferences {
            theme: Theme::Dark,
            brush: paint_rust::brush::Options {
                hardness: 0.25,
                opacity: 0.75,
                antialias: true,
                blend: true,
            },
            ..Default::default()
        };
        for i in 0..20 {
            settings.remember(&PathBuf::from(format!("/tmp/Zażółć {i}\nimage.png")));
        }
        assert_eq!(settings.recent.len(), 16);
        let last = settings.recent[5].clone();
        settings.remember(&last);
        assert_eq!(settings.recent[0], last);
        assert_eq!(settings.recent.len(), 16);
        assert_eq!(Preferences::decode(&settings.encode()).unwrap(), settings);
        for invalid in [
            "preferences-v1\ndark 1 NaN 1 1 0 0\n",
            "preferences-v1\ndark 1 12 2 1 0 0\n",
            "preferences-v1\ndark 1 12 1 1 0 0\nZZ\n",
        ] {
            assert!(Preferences::decode(invalid).is_err());
        }
    }
}
