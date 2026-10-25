use super::*;
impl Editor {
    pub(super) fn show_help(&mut self, ctx: &egui::Context) {
        if !self.help_open {
            return;
        }
        egui::Window::new("Paint Rust help").open(&mut self.help_open).default_width(560.0).vscroll(true).max_height((ctx.content_rect().height()-80.0).max(180.0)).show(ctx,|ui| {
            ui.heading("Paint Rust");
            ui.label(format!("Version {} · net.trelak.PaintRust",env!("CARGO_PKG_VERSION")));
            ui.label("Project license: MIT OR Apache-2.0. Third-party components retain their own licenses; notices accompany the distribution.");
            ui.separator();
            egui::CollapsingHeader::new("User guide").default_open(true).show(ui,|ui| {
                for paragraph in include_str!("../../USER_GUIDE.txt").split("\n\n") {
                    if let Some((heading,body))=paragraph.split_once('\n') {ui.strong(heading);ui.label(body.trim());ui.add_space(6.0);}
                }
            });
            egui::CollapsingHeader::new("Keyboard shortcuts").default_open(true).show(ui,|ui| {
                egui::Grid::new("help-shortcuts").striped(true).show(ui,|ui| {
                    for (keys,action) in [
                        ("Ctrl+N / Ctrl+O","New / Open"),
                        ("Ctrl+S / Ctrl+Shift+S","Save / Save As"),
                        ("Ctrl+Alt+S","Save all"),
                        ("Ctrl+Z / Ctrl+Y","Undo / Redo"),
                        ("Ctrl+Shift+Z","Redo"),
                        ("Ctrl+C / Ctrl+X / Ctrl+V","Copy / Cut / Paste as layer"),
                        ("Ctrl+A / Ctrl+Shift+A","Select all / Deselect"),
                        ("Ctrl+I / Ctrl+Shift+I","Invert selection / Invert colors"),
                        ("Ctrl+Shift+X / Delete","Crop to selection / Delete pixels"),
                        ("Ctrl+M","Merge layer down"),
                        ("F4","Layer properties"),
                        ("Ctrl+Alt+M","Transform selection boundary"),
                        ("Ctrl+F / Ctrl+Shift+F","Repeat effect / Last effect settings"),
                        ("Ctrl+B / Ctrl+Shift+B","Fit image / Fit selection"),
                        ("Ctrl+Tab / Ctrl+Shift+Tab","Next / Previous document"),
                        ("Ctrl+W","Close document"),
                        ("F5 / F6 / F7 / F8","Tools / History / Layers / Colors"),
                        ("Ctrl+Shift+F5…F8","Reset a panel position"),
                        ("Space (hold)","Temporary pan"),
                        ("X","Swap primary and secondary colors"),
                        ("Escape","Cancel preview or active selection gesture"),
                        ("F1","Open help"),
                    ] {ui.label(keys);ui.label(action);ui.end_row();}
                });
                ui.small("Tool shortcuts are shown in tool labels and hover descriptions. Text fields keep their normal editing shortcuts.");
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn help_shortcuts_do_not_modify_the_document_or_selection() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        let selection = Selection::elliptical(1, 1, 10, 10);
        editor.document.set_selection(Some(selection.clone()));
        let pixels = editor.document.shared_pixels();
        for (key, open) in [(Key::F1, true), (Key::Escape, false)] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ctx| {
                    editor.shortcuts(ctx);
                    editor.show_help(ctx);
                },
            );
            assert_eq!(editor.help_open, open);
            assert_eq!(editor.document.selection(), Some(selection.clone()));
            assert_eq!(editor.document.pixels(), pixels.as_slice());
            assert!(!editor.document.dirty());
        }
    }
}
