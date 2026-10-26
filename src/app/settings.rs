use super::*;
use std::io::{Read, Write};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum AfterPick {
    #[default]
    Stay,
    Previous,
    Pencil,
    Brush,
}
impl AfterPick {
    pub const ALL: [Self; 4] = [Self::Stay, Self::Previous, Self::Pencil, Self::Brush];
    pub fn label(self) -> &'static str {
        match self {
            Self::Stay => "Keep color picker",
            Self::Previous => "Previous tool",
            Self::Pencil => "Pencil",
            Self::Brush => "Brush",
        }
    }
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct General {
    version: u8,
    pub ui_scale: f32,
    width: u32,
    height: u32,
    dpi: [f64; 2],
    units: usize,
    transparent: bool,
    editable_shapes: bool,
    pub picker_radius: u8,
    pub picker_merged: bool,
    pub picker_to_brush: bool,
    pub picker_behavior: Option<AfterPick>,
}
impl Default for General {
    fn default() -> Self {
        Self {
            version: 1,
            ui_scale: 1.0,
            width: 960,
            height: 640,
            dpi: [96.0; 2],
            units: 0,
            transparent: false,
            editable_shapes: false,
            picker_radius: 0,
            picker_merged: true,
            picker_to_brush: false,
            picker_behavior: None,
        }
    }
}
impl General {
    pub fn after_pick(&self) -> AfterPick {
        self.picker_behavior.unwrap_or(if self.picker_to_brush {
            AfterPick::Brush
        } else {
            AfterPick::Stay
        })
    }

    fn validate(&self) -> Result<(), String> {
        if self.picker_radius > paint_rust::document::COLOR_SAMPLE_MAX_RADIUS
            || self.version != 1
            || !self.ui_scale.is_finite()
            || !(0.75..=2.5).contains(&self.ui_scale)
            || self.width > 16384
            || self.height > 16384
            || self.units >= paint_rust::resolution::Unit::ALL.len()
            || !(paint_rust::resolution::Dpi {
                x: self.dpi[0],
                y: self.dpi[1],
            })
            .valid()
        {
            return Err("Invalid general preferences".into());
        }
        Document::validate_size(self.width, self.height)
    }
    fn path() -> Option<PathBuf> {
        panels::Layout::path().map(|p| p.with_file_name("general.json"))
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
        let mut bytes = Vec::new();
        file.take(16385)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 16384 {
            return Err("General preferences exceed 16 KiB".into());
        }
        let settings: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        settings.validate()?;
        Ok(settings)
    }
    pub fn save(&self) -> Result<(), String> {
        self.validate()?;
        let path = Self::path().ok_or("No configuration directory")?;
        let parent = path.parent().ok_or("No configuration directory")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }
}
#[derive(Clone)]
pub(super) struct Dialog {
    general: General,
    theme: preferences::Theme,
    grid: bool,
    rulers: bool,
    units: paint_rust::resolution::Unit,
}
impl Editor {
    pub(super) fn current_general_settings(&self) -> General {
        let candidate = General {
            width: self.new_width,
            height: self.new_height,
            dpi: [self.new_dpi.x, self.new_dpi.y],
            units: self.new_units as usize,
            transparent: self.transparent,
            editable_shapes: self.keep_shape_editable,
            ..self.general_settings.clone()
        };
        if candidate.validate().is_ok() {
            candidate
        } else {
            General {
                editable_shapes: self.keep_shape_editable,
                ..self.general_settings.clone()
            }
        }
    }
    pub(super) fn apply_general_settings(
        &mut self,
        settings: General,
        ctx: &egui::Context,
    ) -> Result<(), String> {
        settings.validate()?;
        self.new_width = settings.width;
        self.new_height = settings.height;
        self.new_dpi = paint_rust::resolution::Dpi {
            x: settings.dpi[0],
            y: settings.dpi[1],
        };
        self.new_link_resolution = settings.dpi[0] == settings.dpi[1];
        self.new_units = paint_rust::resolution::Unit::ALL[settings.units];
        self.transparent = settings.transparent;
        self.keep_shape_editable = settings.editable_shapes;
        ctx.set_zoom_factor(settings.ui_scale);
        self.general_settings = settings;
        Ok(())
    }
    pub(super) fn begin_settings(&mut self) {
        self.finish_stroke();
        if self.file_job.is_some() || self.document.editing() {
            return;
        }
        self.settings_dialog = Some(Dialog {
            general: self.current_general_settings(),
            theme: self.preferences.theme,
            grid: self.pixel_grid,
            rulers: self.preferences.rulers,
            units: self.preferences.units,
        });
    }
    pub(super) fn show_settings(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.settings_dialog.take() else {
            return;
        };
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            return;
        }
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Preferences").open(&mut open).collapsible(false).default_width(440.0)
            .anchor(egui::Align2::CENTER_CENTER,Vec2::ZERO).max_height((ctx.content_rect().height()-80.0).max(180.0))
            .show(ctx,|ui| {
                egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-200.0).max(100.0)).show(ui,|ui| {
                    ui.heading("Appearance");
                    ui.horizontal_wrapped(|ui| { for (theme,label) in [(preferences::Theme::System,"System"),(preferences::Theme::Light,"Light"),(preferences::Theme::Dark,"Dark")] {ui.selectable_value(&mut dialog.theme,theme,label);} });
                    ui.add(egui::Slider::new(&mut dialog.general.ui_scale,0.75..=2.5).step_by(0.05).custom_formatter(|v,_|format!("{:.0}%",v*100.0)).text("Interface scale"));
                    ui.small("Interface scale multiplies the system display scale; canvas zoom is separate.");
                    ui.checkbox(&mut dialog.grid,"Pixel grid");ui.checkbox(&mut dialog.rulers,"Rulers");
                    egui::ComboBox::from_label("Ruler units").selected_text(dialog.units.label()).show_ui(ui,|ui| {for unit in paint_rust::resolution::Unit::ALL {ui.selectable_value(&mut dialog.units,unit,unit.label());}});
                    ui.separator();ui.heading("New images");
                    ui.horizontal(|ui| {ui.label("Pixels");ui.add(egui::DragValue::new(&mut dialog.general.width).range(1..=16384).prefix("W "));ui.add(egui::DragValue::new(&mut dialog.general.height).range(1..=16384).prefix("H "));});
                    ui.horizontal_wrapped(|ui| {ui.add(egui::DragValue::new(&mut dialog.general.dpi[0]).range(1.0..=10000.0).prefix("DPI X "));ui.add(egui::DragValue::new(&mut dialog.general.dpi[1]).range(1.0..=10000.0).prefix("DPI Y "));});
                    egui::ComboBox::from_label("New-image units").selected_text(paint_rust::resolution::Unit::ALL[dialog.general.units].label()).show_ui(ui,|ui| {for (index,unit) in paint_rust::resolution::Unit::ALL.into_iter().enumerate() {ui.selectable_value(&mut dialog.general.units,index,unit.label());}});
                    ui.checkbox(&mut dialog.general.transparent,"Transparent new images");
                    ui.small("The last values used in New image are remembered on exit.");
                    ui.separator();ui.checkbox(&mut dialog.general.editable_shapes,"Edit shapes after drawing");
                    if ui.button("Restore defaults").clicked() {dialog=Dialog {general:General::default(),theme:preferences::Theme::System,grid:false,rulers:false,units:Default::default()};}
                });
                let validation=dialog.general.validate();
                if let Err(error)=&validation {ui.colored_label(Color32::LIGHT_RED,error);}
                ui.horizontal(|ui| {apply=ui.add_enabled(validation.is_ok(),egui::Button::new("Apply")).clicked();cancel=ui.button("Cancel").clicked();});
            });
        if apply {
            match self.apply_general_settings(dialog.general, ctx) {
                Ok(()) => {
                    self.preferences.theme = dialog.theme;
                    self.pixel_grid = dialog.grid;
                    self.preferences.pixel_grid = dialog.grid;
                    self.preferences.rulers = dialog.rulers;
                    self.preferences.units = dialog.units;
                    self.apply_theme(ctx);
                    self.preferences.brush_size = self.brush_size;
                    self.preferences.brush = self.brush_options;
                    if let Err(error) = self
                        .general_settings
                        .save()
                        .and_then(|_| self.preferences.save())
                    {
                        self.errors.record("save preferences", &error);
                        self.status =
                            format!("Preferences applied, but could not be saved: {error}");
                    } else {
                        self.status = "Preferences saved".into();
                    }
                }
                Err(error) => {
                    self.errors.record("apply preferences", &error);
                    self.status = error;
                }
            }
        } else if open && !cancel {
            self.settings_dialog = Some(dialog);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_general_settings_round_trip_and_validate_without_touching_documents() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        let pixels = editor.document.shared_pixels();
        let dpi = editor.document.resolution();
        let settings = General {
            ui_scale: 1.25,
            picker_radius: 25,
            picker_merged: false,
            picker_to_brush: true,
            width: 1200,
            height: 800,
            dpi: [300.25, 150.5],
            units: 2,
            editable_shapes: true,
            transparent: true,
            ..Default::default()
        };
        let json = serde_json::to_vec(&settings).unwrap();
        let decoded: General = serde_json::from_slice(&json).unwrap();
        assert_eq!(decoded, settings);
        editor.apply_general_settings(decoded, &ctx).unwrap();
        assert_eq!(editor.current_general_settings(), settings);
        assert_eq!(editor.new_dpi.x, 300.25);
        assert!(editor.keep_shape_editable);
        assert_eq!(editor.document.pixels(), pixels.as_slice());
        assert_eq!(editor.document.resolution(), dpi);
        assert!(!editor.document.dirty());
        let legacy: General = serde_json::from_str("{}").unwrap();
        assert_eq!(legacy.picker_radius, 0);
        assert!(legacy.picker_merged);
        assert!(!legacy.picker_to_brush);
        for invalid in [
            General {
                picker_radius: 26,
                ..settings.clone()
            },
            General {
                ui_scale: f32::NAN,
                ..settings.clone()
            },
            General {
                ui_scale: 5.0,
                ..settings.clone()
            },
            General {
                units: 99,
                ..settings.clone()
            },
            General {
                width: 16384,
                height: 16384,
                ..settings.clone()
            },
            General {
                dpi: [0.0, 96.0],
                ..settings.clone()
            },
        ] {
            assert!(editor.apply_general_settings(invalid, &ctx).is_err());
        }
        assert_eq!(editor.current_general_settings(), settings);
        editor.new_width = 16384;
        editor.new_height = 16384;
        assert_eq!(editor.current_general_settings(), settings);
    }
    #[test]
    fn picker_behavior_preserves_legacy_preferences_and_round_trips_new_modes() {
        let legacy: General = serde_json::from_str(r#"{"picker_to_brush":true}"#).unwrap();
        assert_eq!(legacy.after_pick(), AfterPick::Brush);
        for behavior in AfterPick::ALL {
            let settings = General {
                picker_behavior: Some(behavior),
                ..legacy.clone()
            };
            let restored: General =
                serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
            assert_eq!(restored.after_pick(), behavior);
            restored.validate().unwrap();
        }
    }

    #[test]
    fn preferences_escape_discards_draft_changes() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        let before = editor.current_general_settings();
        editor.begin_settings();
        editor.settings_dialog.as_mut().unwrap().general.ui_scale = 2.0;
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
                ..Default::default()
            },
            |ctx| editor.show_settings(ctx),
        );
        assert!(editor.settings_dialog.is_none());
        assert_eq!(editor.current_general_settings(), before);
        assert!(!editor.document.dirty());
    }
}
