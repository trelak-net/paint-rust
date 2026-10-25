use super::*;
pub(super) struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 4]>,
    pub new_layer: bool,
}
impl Editor {
    fn paste_canvas_size(&self, image: &Image) -> (u32, u32) {
        let (x, y) = self.document.selection().map(|s| (s.x, s.y)).unwrap_or((
            self.document.width.saturating_sub(image.width) / 2,
            self.document.height.saturating_sub(image.height) / 2,
        ));
        (
            self.document.width.max(x + image.width),
            self.document.height.max(y + image.height),
        )
    }
    fn validate_paste_expansion(&self, image: &Image) -> Result<(u32, u32), String> {
        let (width, height) = self.paste_canvas_size(image);
        Document::validate_size(width, height)?;
        let layers = self.document.layers().len() + usize::from(image.new_layer);
        if layers > 32
            || u64::from(width) * u64::from(height) * 4 * layers as u64 > 256 * 1024 * 1024
        {
            return Err("Expanded layers would exceed the document memory limit".into());
        }
        Ok((width, height))
    }
    pub(super) fn receive_paste_image(&mut self, image: Image) {
        if self.paste_canvas_size(&image) != (self.document.width, self.document.height) {
            self.paste_confirmation = Some(image);
            self.status = "Choose how to fit the clipboard image".into();
        } else {
            self.apply_paste_image(image, false);
        }
    }
    pub(super) fn apply_paste_image(&mut self, image: Image, expand: bool) {
        if self.file_job.is_some() {
            return;
        }
        let needed = self.paste_canvas_size(&image);
        let dimensions = if expand {
            match self.validate_paste_expansion(&image) {
                Ok(size) => size,
                Err(error) => {
                    self.status = error;
                    return;
                }
            }
        } else {
            (self.document.width, self.document.height)
        };
        let clipped = needed != dimensions;
        let mut document = self.document.snapshot();
        let selection = self.document.selection();
        document.set_selection(selection.clone());
        self.start_cancellable_file_job("paste image", move |cancel| {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(files::CANCELLED.into());
            }
            if dimensions != (document.width, document.height) {
                document.resize_canvas_cancellable(dimensions.0, dimensions.1, 0, cancel)?;
                document.set_selection(selection);
            }
            if image.new_layer {
                document.paste_layer(image.width, image.height, &image.pixels)?;
            } else {
                document.paste_active_cancellable(
                    image.width,
                    image.height,
                    &image.pixels,
                    cancel,
                )?;
            }
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(files::CANCELLED.into());
            }
            let label = format!(
                "{}{}",
                if image.new_layer {
                    "Paste layer"
                } else {
                    "Paste pixels"
                },
                if expand {
                    " (expanded canvas)"
                } else if clipped {
                    " (clipped to canvas)"
                } else {
                    ""
                }
            );
            Ok(Outcome::Edited(Box::new(document), label, true))
        });
    }
    pub(super) fn show_paste_confirmation(&mut self, ctx: &egui::Context) {
        let Some(image) = self.paste_confirmation.take() else {
            return;
        };
        let mut open = true;
        let mut choice = None;
        let mut cancel = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
        let required = self.paste_canvas_size(&image);
        let valid = self.validate_paste_expansion(&image);
        egui::Window::new("Fit clipboard image")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(420.0)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Clipboard: {} × {} px. Canvas: {} × {} px.",
                    image.width, image.height, self.document.width, self.document.height
                ));
                ui.label("The pasted image extends beyond the current canvas.");
                if ui
                    .add_enabled(
                        valid.is_ok(),
                        egui::Button::new(format!(
                            "Expand canvas to {} × {}",
                            required.0, required.1
                        )),
                    )
                    .clicked()
                {
                    choice = Some(true);
                }
                ui.small(
                    "Existing layer pixels keep their positions; new canvas space is transparent.",
                );
                if let Err(error) = &valid {
                    ui.label(error);
                }
                if ui.button("Keep canvas and clip pasted pixels").clicked() {
                    choice = Some(false);
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        if cancel || !open {
            self.status = "Paste cancelled".into();
        } else if let Some(expand) = choice {
            self.apply_paste_image(image, expand);
        } else {
            self.paste_confirmation = Some(image);
        }
    }
}
