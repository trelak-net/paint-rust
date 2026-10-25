use super::*;

/// Straight RGBA values; display conversion never changes the stored bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Rgba([u8; 4]);
impl Rgba {
    pub const BLACK: Self = Self([0, 0, 0, 255]);
    pub const WHITE: Self = Self([255; 4]);
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 255])
    }
    pub const fn from_rgba_unmultiplied(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }
    pub fn to_srgba_unmultiplied(self) -> [u8; 4] {
        self.0
    }
    pub fn r(self) -> u8 {
        self.0[0]
    }
    pub fn g(self) -> u8 {
        self.0[1]
    }
    pub fn b(self) -> u8 {
        self.0[2]
    }
    fn hsva(self) -> egui::ecolor::Hsva {
        let mut hsv = egui::ecolor::Hsva::from_srgb([self.r(), self.g(), self.b()]);
        hsv.a = f32::from(self.0[3]) / 255.0;
        hsv
    }
    fn from_hsva(hsv: egui::ecolor::Hsva) -> Self {
        Self(hsv.to_srgba_unmultiplied())
    }
    pub fn edit(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let mut hsv = self.hsva();
        let response = egui::color_picker::color_edit_button_hsva(
            ui,
            &mut hsv,
            egui::color_picker::Alpha::OnlyBlend,
        );
        if response.changed() {
            *self = Self::from_hsva(hsv);
        }
        response
    }
}
impl From<Rgba> for Color32 {
    fn from(value: Rgba) -> Self {
        Self::from_rgba_unmultiplied(value.0[0], value.0[1], value.0[2], value.0[3])
    }
}
#[cfg(test)]
impl From<Color32> for Rgba {
    fn from(value: Color32) -> Self {
        Self(value.to_srgba_unmultiplied())
    }
}
#[cfg(test)]
impl PartialEq<Color32> for Rgba {
    fn eq(&self, other: &Color32) -> bool {
        self.0 == other.to_srgba_unmultiplied()
    }
}

pub(super) struct ColorPanel {
    pub palette: Vec<Rgba>,
    secondary: bool,
    hex: String,
    last_color: Option<(bool, Rgba)>,
}
impl Default for ColorPanel {
    fn default() -> Self {
        Self {
            palette: vec![
                Rgba::BLACK,
                Rgba::WHITE,
                Rgba::from_rgb(225, 65, 75),
                Rgba::from_rgb(245, 170, 40),
                Rgba::from_rgb(70, 175, 115),
                Rgba::from_rgb(42, 113, 224),
                Rgba::from_rgb(153, 90, 210),
            ],
            secondary: false,
            hex: String::new(),
            last_color: None,
        }
    }
}
fn parse_hex(text: &str) -> Result<Rgba, String> {
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
    Ok(Rgba::from_rgba_unmultiplied(
        rgba[0], rgba[1], rgba[2], rgba[3],
    ))
}
fn hex(color: Rgba) -> String {
    let c = color.to_srgba_unmultiplied();
    format!("#{:02X}{:02X}{:02X}{:02X}", c[0], c[1], c[2], c[3])
}
fn decode_palette(text: &str) -> Result<Vec<Rgba>, String> {
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
fn encode_palette(colors: &[Rgba]) -> String {
    let mut text = "paint-rust-palette-v1\n".to_string();
    for color in colors {
        text.push_str(&hex(*color));
        text.push('\n');
    }
    text
}
impl ColorPanel {
    fn read(path: &std::path::Path) -> Result<Vec<Rgba>, String> {
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
                *color = Rgba::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
            }
            let hsva = color.hsva();
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
                *color = Rgba::from_hsva(egui::ecolor::Hsva::new(
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
    fn straight_rgba_retains_hidden_rgb_and_all_alpha_values() {
        for alpha in 0..=255 {
            let color = Rgba::from_rgba_unmultiplied(17, 123, 241, alpha);
            assert_eq!(color.to_srgba_unmultiplied(), [17, 123, 241, alpha]);
            assert_eq!(parse_hex(&hex(color)).unwrap(), color);
            assert_eq!(
                decode_palette(&encode_palette(&[color])).unwrap(),
                vec![color]
            );
            assert_eq!(Rgba::from_hsva(color.hsva()), color);
        }
    }
    #[test]
    fn idle_color_controls_do_not_quantize_transparent_colors() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        for alpha in [0, 1, 2, 37, 128] {
            editor.color = Rgba::from_rgba_unmultiplied(17, 123, 241, alpha);
            editor.secondary_color = Rgba::from_rgba_unmultiplied(99, 33, 11, alpha);
            let before = (editor.color, editor.secondary_color);
            for _ in 0..3 {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1180.0, 800.0),
                        )),
                        ..Default::default()
                    },
                    |ctx| editor.show(ctx),
                );
            }
            assert_eq!((editor.color, editor.secondary_color), before);
        }
    }

    #[test]
    fn hex_and_palette_roundtrip_keep_colors_and_alpha() {
        assert_eq!(parse_hex("#12ab34").unwrap(), Rgba::from_rgb(18, 171, 52));
        let colors = vec![Rgba::BLACK, Rgba::WHITE, parse_hex("#FF000080").unwrap()];
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
