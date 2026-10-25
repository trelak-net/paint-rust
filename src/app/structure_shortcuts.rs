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
    pub(super) fn open_resize_dialog(&mut self, canvas: bool) {
        self.finish_stroke();
        self.resize_dialog = Some(ResizeDialog {
            width: self.document.width,
            height: self.document.height,
            canvas,
            lock_ratio: !canvas,
            ratio: f64::from(self.document.width) / f64::from(self.document.height),
            smooth: true,
            anchor: 4,
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
