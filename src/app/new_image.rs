use super::*;
use paint_rust::resolution::Unit;
impl Editor {
    fn create_new_image(&self) -> Result<Document, String> {
        if !self.new_dpi.valid() {
            return Err("Resolution must be between 1 and 10000 DPI".into());
        }
        let mut document = Document::new(
            self.new_width,
            self.new_height,
            if self.transparent { [0; 4] } else { [255; 4] },
        )?;
        paint_rust::resolution::write(&mut document.metadata, self.new_dpi)?;
        Ok(document)
    }
    pub(super) fn show_new_image(&mut self, ctx: &egui::Context) {
        if !self.new_dialog {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.new_dialog = false;
            return;
        }
        let mut open = true;
        egui::Window::new("New image")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .max_height((ctx.content_rect().height() - 80.0).max(180.0))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 200.0).max(100.0))
                    .show(ui, |ui| {
                        egui::ComboBox::from_label("Measurement units")
                            .selected_text(self.new_units.label())
                            .show_ui(ui, |ui| {
                                for unit in Unit::ALL {
                                    ui.selectable_value(&mut self.new_units, unit, unit.label());
                                }
                            });
                        for axis in 0..2 {
                            let factor = self.new_units.pixels_per_unit(if axis == 0 {
                                self.new_dpi.x
                            } else {
                                self.new_dpi.y
                            });
                            let dimension = if axis == 0 {
                                &mut self.new_width
                            } else {
                                &mut self.new_height
                            };
                            let mut value = f64::from(*dimension) / factor;
                            ui.horizontal(|ui| {
                                let label = ui.label(if axis == 0 { "Width" } else { "Height" });
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut value)
                                            .range(1.0 / factor..=16384.0 / factor)
                                            .speed(if self.new_units == Unit::Pixels {
                                                1.0
                                            } else {
                                                0.01
                                            })
                                            .suffix(format!(" {}", self.new_units.suffix())),
                                    )
                                    .labelled_by(label.id)
                                    .changed()
                                {
                                    *dimension =
                                        (value * factor).round().clamp(1.0, 16384.0) as u32;
                                }
                            });
                        }
                        ui.label(format!(
                            "Pixel size: {} × {}",
                            self.new_width, self.new_height
                        ));
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add(
                                    egui::DragValue::new(&mut self.new_dpi.x)
                                        .range(1.0..=10000.0)
                                        .prefix("DPI X "),
                                )
                                .changed()
                                && self.new_link_resolution
                            {
                                self.new_dpi.y = self.new_dpi.x;
                            }
                            if ui
                                .add(
                                    egui::DragValue::new(&mut self.new_dpi.y)
                                        .range(1.0..=10000.0)
                                        .prefix("DPI Y "),
                                )
                                .changed()
                                && self.new_link_resolution
                            {
                                self.new_dpi.x = self.new_dpi.y;
                            }
                        });
                        ui.checkbox(&mut self.new_link_resolution, "Link X/Y resolution");
                        ui.small(
                            "Changing DPI keeps pixel dimensions and changes physical print size.",
                        );
                        ui.checkbox(&mut self.transparent, "Transparent background");
                        ui.small(format!(
                            "Layer pixels: {:.1} MiB",
                            f64::from(self.new_width) * f64::from(self.new_height) * 4.0
                                / (1024.0 * 1024.0)
                        ));
                    });
                let validation = Document::validate_size(self.new_width, self.new_height);
                if let Err(error) = &validation {
                    ui.colored_label(Color32::LIGHT_RED, error);
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            validation.is_ok() && self.new_dpi.valid(),
                            egui::Button::new("Create"),
                        )
                        .clicked()
                    {
                        match self.create_new_image() {
                            Ok(document) => {
                                self.install_document(document, None);
                                self.new_dialog = false;
                                self.status = "New image created".into();
                            }
                            Err(error) => {
                                self.errors.record("create image", &error);
                                self.status = error;
                            }
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        self.new_dialog = false;
                    }
                });
            });
        self.new_dialog &= open;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_image_has_requested_density_alpha_and_no_artificial_history() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.new_width = 30;
        editor.new_height = 15;
        editor.new_dpi = paint_rust::resolution::Dpi {
            x: 300.25,
            y: 150.5,
        };
        editor.transparent = true;
        let document = editor.create_new_image().unwrap();
        assert_eq!((document.width, document.height), (30, 15));
        assert_eq!(document.resolution(), editor.new_dpi);
        assert!(document.pixels().iter().all(|pixel| *pixel == [0; 4]));
        assert!(!document.dirty());
        assert!(!document.can_undo());
        editor.new_width = 16384;
        editor.new_height = 16384;
        assert!(editor.create_new_image().is_err());
        editor.new_dpi.x = f64::NAN;
        assert!(editor.create_new_image().is_err());
    }
    #[test]
    fn cancelling_physical_size_dialog_leaves_current_document_untouched() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        let pixels = editor.document.shared_pixels();
        editor.new_dialog = true;
        editor.new_units = Unit::Inches;
        editor.new_dpi.x = 300.0;
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| {
                editor.shortcuts(ctx);
                editor.show_new_image(ctx);
            },
        );
        assert!(!editor.new_dialog);
        assert_eq!(editor.document.pixels(), pixels.as_slice());
        assert!(!editor.document.dirty());
    }
}
