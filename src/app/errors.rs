use super::*;
use std::collections::VecDeque;

#[derive(Default)]
pub(super) struct ErrorLog {
    entries: VecDeque<String>,
    pub open: bool,
}
impl ErrorLog {
    pub fn record(&mut self, operation: &str, detail: &str) {
        if detail == files::CANCELLED {
            return;
        }
        let mut text: String = operation
            .chars()
            .chain("\n\n".chars())
            .chain(detail.chars())
            .take(16_384)
            .collect();
        if text.chars().count() == 16_384 {
            text.push_str("\n[Details truncated]");
        }
        if self.entries.back() == Some(&text) {
            return;
        }
        if self.entries.len() == 16 {
            self.entries.pop_front();
        }
        self.entries.push_back(text);
    }
    pub fn has_entries(&self) -> bool {
        !self.entries.is_empty()
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let mut open = true;
        let mut clear = false;
        egui::Window::new("Operation errors")
            .open(&mut open)
            .default_width(560.0)
            .max_height((ctx.content_rect().height() - 80.0).max(180.0))
            .show(ctx, |ui| {
                ui.label("Recent operation errors from this session, newest first.");
                ui.small("Details may contain file paths. Review them before sharing.");
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(self.has_entries(), egui::Button::new("Copy all details"))
                        .clicked()
                    {
                        ctx.copy_text(
                            self.entries
                                .iter()
                                .rev()
                                .cloned()
                                .collect::<Vec<_>>()
                                .join("\n\n---\n\n"),
                        );
                    }
                    clear = ui
                        .add_enabled(self.has_entries(), egui::Button::new("Clear"))
                        .clicked();
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 230.0).max(80.0))
                    .show(ui, |ui| {
                        if !self.has_entries() {
                            ui.label("No recorded operation errors.");
                        }
                        for (index, entry) in self.entries.iter().rev().enumerate() {
                            ui.push_id(index, |ui| {
                                let mut text = entry.as_str();
                                ui.add(
                                    egui::TextEdit::multiline(&mut text)
                                        .desired_width(f32::INFINITY),
                                );
                                if ui.button("Copy details").clicked() {
                                    ctx.copy_text(entry.clone());
                                }
                                ui.separator();
                            });
                        }
                    });
            });
        if clear {
            self.entries.clear();
        }
        self.open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn errors_are_bounded_deduplicated_and_cancellation_is_not_an_error() {
        let mut log = ErrorLog::default();
        log.record("save", files::CANCELLED);
        assert!(!log.has_entries());
        for i in 0..20 {
            log.record("save", &format!("failure {i}"));
        }
        log.record("save", "failure 19");
        assert_eq!(log.entries.len(), 16);
        assert!(log.entries.front().unwrap().ends_with("failure 4"));
        log.record("open", &"ą".repeat(100_000));
        assert!(log.entries.back().unwrap().ends_with("[Details truncated]"));
        assert!(log.entries.back().unwrap().len() < 66_000);
    }
    #[test]
    fn error_dialog_and_escape_leave_document_and_selection_unchanged() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        let pixels = editor.document.shared_pixels();
        editor
            .errors
            .record("save project", "No space left on device");
        editor.errors.open = true;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                ..Default::default()
            },
            |ctx| editor.errors.show(ctx),
        );
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
            |ctx| editor.shortcuts(ctx),
        );
        assert!(!editor.errors.open);
        assert!(editor.errors.has_entries());
        assert_eq!(editor.document.pixels(), pixels.as_slice());
        assert!(!editor.document.dirty());
    }
}
