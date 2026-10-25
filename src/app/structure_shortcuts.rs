use super::*;
#[derive(Clone, Copy)]
enum Command {
    Resize(bool),
    Rotate(Orientation),
    Add(bool),
    Delete,
    Visibility,
    Select { up: bool, edge: bool },
    Flatten,
}
impl Editor {
    pub(super) fn apply_resize_dialog(&mut self, dialog: &ResizeDialog) -> bool {
        let (width, height, canvas, anchor, resampling, dpi) = (
            dialog.width,
            dialog.height,
            dialog.canvas,
            dialog.anchor,
            dialog.resampling,
            dialog.dpi,
        );
        let fill = match dialog.fill {
            1 => self.color.to_srgba_unmultiplied(),
            2 => self.secondary_color.to_srgba_unmultiplied(),
            3 => [255; 4],
            4 => [0, 0, 0, 255],
            _ => [0; 4],
        };
        self.transform_in_background("resize image", move |document, cancel| {
            let resolution_changed = document.set_resolution(dpi)?;
            let geometry_changed = if canvas {
                document.resize_canvas_filled_cancellable(width, height, anchor, fill, cancel)
            } else {
                document.resize_image_with_options_cancellable(width, height, resampling, cancel)
            }?;
            Ok(resolution_changed || geometry_changed)
        })
    }

    pub(super) fn open_resize_dialog(&mut self, canvas: bool) {
        self.finish_stroke();
        self.resize_dialog = Some(ResizeDialog {
            width: self.document.width,
            height: self.document.height,
            canvas,
            lock_ratio: !canvas,
            ratio: f64::from(self.document.width) / f64::from(self.document.height),
            resampling: paint_rust::resample::Options {
                gamma_correct: true,
                ..Default::default()
            },
            units: self.preferences.units,
            dpi: self.document.resolution(),
            link_resolution: self.document.resolution().x == self.document.resolution().y,
            anchor: 4,
            fill: 0,
            percentage: false,
            percent: 100.0,
            original_size: (self.document.width, self.document.height),
        });
    }
    pub(super) fn structure_shortcuts(&mut self, ctx: &egui::Context) -> bool {
        if self.document.editing() || self.last_point.is_some() {
            return false;
        }
        use egui::Modifiers as M;
        let command = ctx.input_mut(|i| {
            [
                (M::COMMAND | M::SHIFT, Key::R, Command::Resize(true)),
                (M::COMMAND, Key::R, Command::Resize(false)),
                (M::COMMAND, Key::H, Command::Rotate(Orientation::Clockwise)),
                (
                    M::COMMAND,
                    Key::G,
                    Command::Rotate(Orientation::Counterclockwise),
                ),
                (M::COMMAND | M::SHIFT, Key::N, Command::Add(false)),
                (M::COMMAND | M::SHIFT, Key::D, Command::Add(true)),
                (M::COMMAND | M::SHIFT, Key::Delete, Command::Delete),
                (M::COMMAND, Key::Comma, Command::Visibility),
                (M::COMMAND | M::SHIFT, Key::F, Command::Flatten),
                (
                    M::COMMAND | M::ALT,
                    Key::PageUp,
                    Command::Select {
                        up: true,
                        edge: true,
                    },
                ),
                (
                    M::COMMAND | M::ALT,
                    Key::PageDown,
                    Command::Select {
                        up: false,
                        edge: true,
                    },
                ),
                (
                    M::ALT,
                    Key::PageUp,
                    Command::Select {
                        up: true,
                        edge: false,
                    },
                ),
                (
                    M::ALT,
                    Key::PageDown,
                    Command::Select {
                        up: false,
                        edge: false,
                    },
                ),
            ]
            .into_iter()
            .find_map(|(modifiers, key, command)| i.consume_key(modifiers, key).then_some(command))
        });
        let Some(command) = command else {
            return false;
        };
        match command {
            Command::Resize(canvas) => self.open_resize_dialog(canvas),
            Command::Rotate(operation) => {
                self.transform_in_background("transform image", move |document, cancel| {
                    document.reorient_cancellable(operation, cancel)
                });
            }
            Command::Add(duplicate) => {
                self.texture_dirty |= self.document.add_layer(duplicate);
            }
            Command::Delete => {
                self.texture_dirty |= self.document.remove_layer();
            }
            Command::Visibility => {
                let index = self.document.active_layer();
                self.document
                    .set_visibility(index, !self.document.layers()[index].visible);
                self.texture_dirty = true;
            }
            Command::Flatten => {
                if self.document.can_flatten() {
                    self.start_document_edit(DocumentEdit::Flatten);
                }
            }
            Command::Select { up, edge } => {
                let last = self.document.layers().len() - 1;
                let active = self.document.active_layer();
                let target = match (up, edge) {
                    (true, true) => last,
                    (false, true) => 0,
                    (true, false) => (active + 1).min(last),
                    (false, false) => active.saturating_sub(1),
                };
                self.document.select_layer(target);
            }
        }
        true
    }
}

impl ResizeDialog {
    pub(super) fn apply_percentage(&mut self) {
        let maximum = 1638400.0 / f64::from(self.original_size.0.max(self.original_size.1));
        self.percent = self.percent.clamp(0.01, maximum);
        self.width = (f64::from(self.original_size.0) * self.percent / 100.0)
            .round()
            .clamp(1.0, 16384.0) as u32;
        self.height = (f64::from(self.original_size.1) * self.percent / 100.0)
            .round()
            .clamp(1.0, 16384.0) as u32;
    }
}
