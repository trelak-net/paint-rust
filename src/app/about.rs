use super::*;

#[derive(Default)]
pub(super) struct About {
    pub open: bool,
    license: usize,
}
impl About {
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let summary = format!(
            "Paint Rust {}\nnet.trelak.PaintRust\n{} / {}\nProject license: MIT OR Apache-2.0",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        egui::Window::new("About Paint Rust")
            .open(&mut self.open)
            .default_width(560.0)
            .max_height((ctx.content_rect().height() - 80.0).max(180.0))
            .show(ctx, |ui| {
                ui.heading("Paint Rust");
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.label("A native desktop raster editor");
                ui.label("Package: net.trelak.PaintRust");
                ui.label(format!("Platform: {} / {}", std::env::consts::OS, std::env::consts::ARCH));
                if ui.button("Copy version information").clicked() { ctx.copy_text(summary); }
                ui.separator();
                ui.label("Project code and artwork: MIT OR Apache-2.0.");
                ui.small("Third-party components retain their own licenses. Full dependency notices accompany the application distribution.");
                ui.horizontal_wrapped(|ui| {
                    for (index, label) in ["MIT", "Apache 2.0", "Third-party notices"].iter().enumerate() {
                        ui.selectable_value(&mut self.license, index, *label);
                    }
                });
                egui::ScrollArea::vertical()
                    .id_salt(("about-license", self.license))
                    .max_height((ctx.content_rect().height() - 390.0).max(70.0))
                    .show(ui, |ui| {
                        let mut text = match self.license {
                            1 => include_str!("../../LICENSE-APACHE"),
                            2 => include_str!("../../THIRD_PARTY_NOTICES"),
                            _ => include_str!("../../LICENSE-MIT"),
                        };
                        ui.add(egui::TextEdit::multiline(&mut text).desired_width(f32::INFINITY));
                    });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn about_license_tabs_and_escape_do_not_change_the_document() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        let pixels = editor.document.shared_pixels();
        editor.about.open = true;
        for license in 0..3 {
            editor.about.license = license;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                    ..Default::default()
                },
                |ctx| editor.about.show(ctx),
            );
        }
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
        assert!(!editor.about.open);
        assert_eq!(editor.document.pixels(), pixels.as_slice());
        assert!(!editor.document.dirty());
    }
}
