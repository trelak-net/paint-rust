use super::*;

pub(super) struct ColorPanel {
    pub palette: Vec<Color32>,
    secondary: bool,
    hex: String,
    last_color: Option<(bool, Color32)>,
}
impl Default for ColorPanel {
    fn default() -> Self {
        Self {
            palette: vec![
                Color32::BLACK,
                Color32::WHITE,
                Color32::from_rgb(225, 65, 75),
                Color32::from_rgb(245, 170, 40),
                Color32::from_rgb(70, 175, 115),
                Color32::from_rgb(42, 113, 224),
                Color32::from_rgb(153, 90, 210),
            ],
            secondary: false,
            hex: String::new(),
            last_color: None,
        }
    }
}
fn parse_hex(text: &str) -> Result<Color32, String> {
    let text = text.trim().strip_prefix('#').unwrap_or(text.trim());
    if text.len() != 6 && text.len() != 8 {
        return Err("Use RRGGBB or RRGGBBAA".into());
    }
    if !text.is_ascii() {
        return Err("Invalid hexadecimal color".into());
    }
    let mut rgba = [255; 4];
    for (index, value) in rgba.iter_mut().enumerate().take(text.len() / 2) {
        *value = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| "Invalid hexadecimal color")?;
    }
    Ok(Color32::from_rgba_unmultiplied(
        rgba[0], rgba[1], rgba[2], rgba[3],
    ))
}
fn hex(color: Color32) -> String {
    let c = color.to_srgba_unmultiplied();
    format!("#{:02X}{:02X}{:02X}{:02X}", c[0], c[1], c[2], c[3])
}
fn decode_palette(text: &str) -> Result<Vec<Color32>, String> {
    if text.len() > 8192 {
        return Err("Palette exceeds size limit".into());
    }
    let mut lines = text.lines();
    if lines.next() != Some("paint-rust-palette-v1") {
        return Err("Unsupported palette file".into());
    }
    let colors: Result<Vec<_>, _> = lines
        .filter(|line| !line.trim().is_empty())
        .map(parse_hex)
        .collect();
    let colors = colors?;
    if colors.is_empty() || colors.len() > 256 {
        return Err("A palette must contain 1 to 256 colors".into());
    }
    Ok(colors)
}
fn encode_palette(colors: &[Color32]) -> String {
    let mut text = "paint-rust-palette-v1\n".to_string();
    for color in colors {
        text.push_str(&hex(*color));
        text.push('\n');
    }
    text
}
impl ColorPanel {
    fn read(path: &std::path::Path) -> Result<Vec<Color32>, String> {
        use std::io::Read;
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut text = String::new();
        file.take(8193)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        decode_palette(&text)
    }
    fn write(&self, path: &std::path::Path) -> Result<(), String> {
        use std::io::Write;
        let text = encode_palette(&self.palette);
        decode_palette(&text)?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut output = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        output
            .write_all(text.as_bytes())
            .map_err(|e| e.to_string())?;
        output.as_file().sync_all().map_err(|e| e.to_string())?;
        output.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub(super) fn load(&mut self) -> Result<(), String> {
        if let Some(path) = super::panels::Layout::path().map(|p| p.with_file_name("palette.txt"))
            && path.exists()
        {
            self.palette = Self::read(&path)?;
        }
        Ok(())
    }
    pub(super) fn save(&self) -> Result<(), String> {
        let path = super::panels::Layout::path()
            .ok_or("No configuration directory")?
            .with_file_name("palette.txt");
        self.write(&path)
    }
}
impl Editor {
    pub(super) fn color_channels(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Color channels").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.color_panel.secondary, false, "Primary");
                ui.selectable_value(&mut self.color_panel.secondary, true, "Secondary");
            });
            let color = if self.color_panel.secondary {
                &mut self.secondary_color
            } else {
                &mut self.color
            };
            let mut rgba = color.to_srgba_unmultiplied();
            let mut changed = false;
            egui::Grid::new("rgba-inputs")
                .num_columns(2)
                .show(ui, |ui| {
                    for (name, value) in ["Red", "Green", "Blue", "Alpha"]
                        .into_iter()
                        .zip(rgba.iter_mut())
                    {
                        let label = ui.label(name);
                        changed |= ui
                            .add(egui::DragValue::new(value).range(0..=255))
                            .labelled_by(label.id)
                            .changed();
                        ui.end_row();
                    }
                });
            if changed {
                *color = Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
            }
            let hsva = egui::ecolor::Hsva::from(*color);
            let mut hsv = [hsva.h * 360.0, hsva.s * 100.0, hsva.v * 100.0];
            let mut changed = false;
            egui::Grid::new("hsv-inputs").num_columns(2).show(ui, |ui| {
                for (index, (name, value)) in ["Hue", "Saturation", "Value"]
                    .into_iter()
                    .zip(hsv.iter_mut())
                    .enumerate()
                {
                    let label = ui.label(name);
                    changed |= ui
                        .add(
                            egui::DragValue::new(value)
                                .range(0.0..=if index == 0 { 360.0 } else { 100.0 })
                                .speed(0.5),
                        )
                        .labelled_by(label.id)
                        .changed();
                    ui.end_row();
                }
            });
            if changed {
                *color = Color32::from(egui::ecolor::Hsva::new(
                    hsv[0] / 360.0,
                    hsv[1] / 100.0,
                    hsv[2] / 100.0,
                    hsva.a,
                ));
            }
            let key = (self.color_panel.secondary, *color);
            if self.color_panel.last_color != Some(key) {
                self.color_panel.hex = hex(*color);
                self.color_panel.last_color = Some(key);
            }
            let label = ui.label("Hex RGBA");
            let edit = ui
                .text_edit_singleline(&mut self.color_panel.hex)
                .labelled_by(label.id);
            let parsed = parse_hex(&self.color_panel.hex);
            if ui
                .add_enabled(parsed.is_ok(), egui::Button::new("Apply hex"))
                .clicked()
                || (edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
            {
                if let Ok(parsed) = parsed {
                    *color = parsed;
                    self.color_panel.last_color = Some((self.color_panel.secondary, *color));
                }
            } else if let Err(error) = parsed {
                ui.small(error);
            }
        });
    }
    pub(super) fn palette_actions(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    self.color_panel.palette.len() < 256,
                    egui::Button::new("Add current"),
                )
                .clicked()
            {
                self.color_panel
                    .palette
                    .push(if self.color_panel.secondary {
                        self.secondary_color
                    } else {
                        self.color
                    });
            }
            ui.menu_button("Palette", |ui| {
                if ui.button("Load…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Color palette", &["palette", "txt"])
                        .pick_file()
                    {
                        match ColorPanel::read(&path) {
                            Ok(colors) => self.color_panel.palette = colors,
                            Err(e) => self.status = format!("Could not load palette: {e}"),
                        }
                    }
                    ui.close();
                }
                if ui.button("Save as…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Color palette", &["palette"])
                        .set_file_name("colors.palette")
                        .save_file()
                        && let Err(e) = self.color_panel.write(&path)
                    {
                        self.status = format!("Could not save palette: {e}");
                    }
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.color_panel.palette.len() > 1,
                        egui::Button::new("Remove last color"),
                    )
                    .clicked()
                {
                    self.color_panel.palette.pop();
                    ui.close();
                }
                if ui.button("Reset palette").clicked() {
                    self.color_panel.palette = ColorPanel::default().palette;
                    ui.close();
                }
            });
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hex_and_palette_roundtrip_keep_colors_and_alpha() {
        assert_eq!(
            parse_hex("#12ab34").unwrap(),
            Color32::from_rgb(18, 171, 52)
        );
        let colors = vec![
            Color32::BLACK,
            Color32::WHITE,
            parse_hex("#FF000080").unwrap(),
        ];
        assert_eq!(decode_palette(&encode_palette(&colors)).unwrap(), colors);
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("colors.palette");
        let panel = ColorPanel {
            palette: colors.clone(),
            ..Default::default()
        };
        panel.write(&path).unwrap();
        assert_eq!(ColorPanel::read(&path).unwrap(), colors);
    }
    #[test]
    fn malformed_palette_cannot_replace_current_colors() {
        for text in ["", "#1", "#nothex", "#123456789", "é12345"] {
            assert!(parse_hex(text).is_err());
        }
        assert!(decode_palette("paint-rust-palette-v1\n").is_err());
        assert!(
            decode_palette(&format!(
                "paint-rust-palette-v1\n{}",
                "#FFFFFF\n".repeat(257)
            ))
            .is_err()
        );
        assert!(decode_palette("paint-rust-palette-v1\n#FFFFFG\n").is_err());
    }
}
