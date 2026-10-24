use eframe::egui::{self, Color32, Key, Pos2, Rect, Sense, TextureHandle, TextureOptions, Vec2};
use paint_rust::{document::Document, files};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Brush,
    Eraser,
    Fill,
    Picker,
}

#[derive(Clone, Copy)]
enum Action {
    New,
    Open,
    Close,
}

pub struct Editor {
    document: Document,
    texture: Option<TextureHandle>,
    texture_dirty: bool,
    tool: Tool,
    color: Color32,
    brush_size: f32,
    zoom: f32,
    last_point: Option<(f32, f32)>,
    path: Option<PathBuf>,
    status: String,
    pending: Option<Action>,
    new_dialog: bool,
    new_width: u32,
    new_height: u32,
    transparent: bool,
    allow_close: bool,
    suppress_stroke: bool,
    appearance: Option<std::sync::mpsc::Receiver<egui::Theme>>,
}

impl Editor {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut editor = Self::blank(&cc.egui_ctx);
        editor.appearance = Some(crate::appearance::connect(&cc.egui_ctx));
        if let Some(path) = std::env::args_os().nth(1) {
            editor.load(PathBuf::from(path));
        }
        editor
    }

    fn blank(ctx: &egui::Context) -> Self {
        crate::appearance::configure(ctx);
        Self {
            document: Document::new(960, 640, [255; 4]).unwrap(),
            texture: None,
            texture_dirty: true,
            tool: Tool::Brush,
            color: Color32::from_rgb(42, 113, 224),
            brush_size: 12.0,
            zoom: 1.0,
            last_point: None,
            path: None,
            status: "Ready — draw on the canvas to begin".into(),
            pending: None,
            new_dialog: false,
            new_width: 960,
            new_height: 640,
            transparent: false,
            allow_close: false,
            suppress_stroke: false,
            appearance: None,
        }
    }

    fn load(&mut self, path: PathBuf) {
        match files::open(&path) {
            Ok(document) => {
                self.document = document;
                self.path = Some(path);
                self.texture_dirty = true;
                self.last_point = None;
                self.zoom = 1.0;
                self.status = "Image opened".into();
            }
            Err(error) => self.status = format!("Could not open image: {error}"),
        }
    }

    fn save(&mut self, save_as: bool) -> bool {
        self.finish_stroke();
        let existing = self
            .path
            .as_ref()
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")));
        let path = if !save_as && existing.is_some() {
            existing.cloned()
        } else {
            rfd::FileDialog::new()
                .add_filter("PNG image", &["png"])
                .set_file_name(
                    self.path
                        .as_ref()
                        .and_then(|p| p.file_stem())
                        .and_then(|s| s.to_str())
                        .map(|s| format!("{s}.png"))
                        .unwrap_or_else(|| "Untitled.png".into()),
                )
                .save_file()
                .map(|p| {
                    if p.extension().is_none() {
                        p.with_extension("png")
                    } else {
                        p
                    }
                })
        };
        let Some(path) = path else {
            return false;
        };
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("png"))
        {
            self.status = "Could not save image: choose a filename ending in .png".into();
            return false;
        }
        match files::save_png(&self.document, &path) {
            Ok(()) => {
                self.document.mark_saved();
                self.status = format!("Saved {}", path.display());
                self.path = Some(path);
                true
            }
            Err(error) => {
                self.status = format!("Could not save image: {error}");
                false
            }
        }
    }

    fn request(&mut self, action: Action, ctx: &egui::Context) {
        self.finish_stroke();
        if self.document.dirty() {
            self.pending = Some(action);
        } else {
            self.perform(action, ctx);
        }
    }

    fn perform(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::New => self.new_dialog = true,
            Action::Open => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "webp"])
                    .pick_file()
                {
                    self.load(path);
                }
            }
            Action::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn finish_stroke(&mut self) {
        if self.document.editing() {
            self.document.commit();
            self.texture_dirty = true;
        }
        self.last_point = None;
    }

    fn undo(&mut self) {
        self.finish_stroke();
        self.document.undo();
        self.texture_dirty = true;
    }
    fn redo(&mut self) {
        self.finish_stroke();
        self.document.redo();
        self.texture_dirty = true;
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let command = ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::COMMAND, Key::N) {
                Some(0)
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::O) {
                Some(1)
            } else if i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::S) {
                Some(2)
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::S) {
                Some(3)
            } else if i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::Z)
                || i.consume_key(egui::Modifiers::COMMAND, Key::Y)
            {
                Some(4)
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::Z) {
                Some(5)
            } else if i.key_pressed(Key::Escape) {
                Some(6)
            } else if !i.modifiers.command && !self.document.editing() {
                if i.key_pressed(Key::B) {
                    self.tool = Tool::Brush;
                }
                if i.key_pressed(Key::E) {
                    self.tool = Tool::Eraser;
                }
                if i.key_pressed(Key::F) {
                    self.tool = Tool::Fill;
                }
                if i.key_pressed(Key::I) {
                    self.tool = Tool::Picker;
                }
                None
            } else {
                None
            }
        });
        match command {
            Some(0) => self.request(Action::New, ctx),
            Some(1) => self.request(Action::Open, ctx),
            Some(2) => {
                self.save(true);
            }
            Some(3) => {
                self.save(false);
            }
            Some(4) => self.redo(),
            Some(5) => self.undo(),
            Some(6) => {
                self.suppress_stroke = true;
                self.document.cancel();
                self.last_point = None;
                self.texture_dirty = true;
            }
            _ => {}
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        if self.texture_dirty {
            let bytes: Vec<u8> = self.document.pixels().iter().flatten().copied().collect();
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [self.document.width as usize, self.document.height as usize],
                &bytes,
            );
            if let Some(texture) = &mut self.texture {
                texture.set(image, TextureOptions::NEAREST);
            } else {
                self.texture = Some(ui.ctx().load_texture(
                    "canvas",
                    image,
                    TextureOptions::NEAREST,
                ));
            }
            self.texture_dirty = false;
        }
        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let size =
                    Vec2::new(self.document.width as f32, self.document.height as f32) * self.zoom;
                let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
                let clip = rect.intersect(ui.clip_rect());
                let painter = ui.painter().with_clip_rect(clip);
                painter.rect_filled(rect, 0.0, Color32::from_gray(210));
                let tile = 12.0;
                let x_start = ((clip.min.x - rect.min.x) / tile).floor() as i32;
                let y_start = ((clip.min.y - rect.min.y) / tile).floor() as i32;
                let x_end = ((clip.max.x - rect.min.x) / tile).ceil() as i32;
                let y_end = ((clip.max.y - rect.min.y) / tile).ceil() as i32;
                for y in y_start..y_end {
                    for x in x_start..x_end {
                        if (x + y) % 2 == 0 {
                            painter.rect_filled(
                                Rect::from_min_size(
                                    rect.min + Vec2::new(x as f32, y as f32) * tile,
                                    Vec2::splat(tile),
                                ),
                                0.0,
                                Color32::from_gray(170),
                            );
                        }
                    }
                }
                if let Some(texture) = &self.texture {
                    painter.image(
                        texture.id(),
                        rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                if response.hovered() && matches!(self.tool, Tool::Brush | Tool::Eraser) {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                    if let Some(pos) = response.hover_pos() {
                        let radius = self.brush_size * self.zoom / 2.0;
                        painter.circle_stroke(
                            pos,
                            radius,
                            egui::Stroke::new(2.0_f32, Color32::BLACK),
                        );
                        painter.circle_stroke(
                            pos,
                            radius + 1.0,
                            egui::Stroke::new(1.0_f32, Color32::WHITE),
                        );
                    }
                }
                if ui.is_enabled()
                    && !self.suppress_stroke
                    && ((response.is_pointer_button_down_on()
                        && ui.input(|i| i.pointer.primary_down()))
                        || response.clicked_by(egui::PointerButton::Primary))
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    let local = (pos - rect.min) / self.zoom;
                    let point = (local.x, local.y);
                    if self.last_point.is_none() {
                        match self.tool {
                            Tool::Picker => {
                                if let Some(pixel) = self
                                    .document
                                    .sample(local.x.floor() as i32, local.y.floor() as i32)
                                {
                                    self.color = Color32::from_rgba_unmultiplied(
                                        pixel[0], pixel[1], pixel[2], pixel[3],
                                    );
                                }
                            }
                            Tool::Fill => {
                                self.document.begin("Flood fill");
                                self.document.fill(
                                    local.x.floor() as i32,
                                    local.y.floor() as i32,
                                    self.color.to_srgba_unmultiplied(),
                                );
                                self.document.commit();
                                self.texture_dirty = true;
                            }
                            _ => self.document.begin(if self.tool == Tool::Eraser {
                                "Eraser"
                            } else {
                                "Brush stroke"
                            }),
                        }
                    }
                    if matches!(self.tool, Tool::Brush | Tool::Eraser) {
                        self.document.stroke(
                            self.last_point.unwrap_or(point),
                            point,
                            self.brush_size,
                            self.color.to_srgba_unmultiplied(),
                            self.tool == Tool::Eraser,
                        );
                        self.texture_dirty = true;
                    }
                    self.last_point = Some(point);
                    if !ui.input(|i| i.pointer.primary_down()) {
                        self.finish_stroke();
                    }
                }
            });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(action) = self.pending {
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("Save your changes before continuing?");
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() && self.save(false) {
                            self.pending = None;
                            self.perform(action, ctx);
                        }
                        if ui.button("Discard").clicked() {
                            self.pending = None;
                            self.perform(action, ctx);
                        }
                        if ui.button("Cancel").clicked() {
                            self.pending = None;
                        }
                    });
                    if self.status.starts_with("Could not") {
                        ui.colored_label(Color32::LIGHT_RED, &self.status);
                    }
                });
        }
        if self.new_dialog {
            egui::Window::new("New image")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Width");
                        ui.add(
                            egui::DragValue::new(&mut self.new_width)
                                .range(1..=4096)
                                .suffix(" px"),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Height");
                        ui.add(
                            egui::DragValue::new(&mut self.new_height)
                                .range(1..=4096)
                                .suffix(" px"),
                        );
                    });
                    ui.checkbox(&mut self.transparent, "Transparent background");
                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            match Document::new(
                                self.new_width,
                                self.new_height,
                                if self.transparent { [0; 4] } else { [255; 4] },
                            ) {
                                Ok(document) => {
                                    self.document = document;
                                    self.path = None;
                                    self.texture_dirty = true;
                                    self.zoom = 1.0;
                                    self.new_dialog = false;
                                    self.status = "New image created".into();
                                }
                                Err(error) => self.status = error,
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.new_dialog = false;
                        }
                    });
                });
        }
    }
}

impl Editor {
    fn show(&mut self, ctx: &egui::Context) {
        if let Some(receiver) = &self.appearance
            && let Some(theme) = receiver.try_iter().last()
        {
            ctx.set_theme(theme);
            ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(match theme {
                egui::Theme::Light => egui::SystemTheme::Light,
                egui::Theme::Dark => egui::SystemTheme::Dark,
            }));
        }
        if !ctx.input(|i| i.pointer.primary_down() && i.focused) {
            self.finish_stroke();
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && self.document.dirty()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Action::Close);
        }
        let enabled = self.pending.is_none() && !self.new_dialog;
        if enabled {
            self.shortcuts(ctx);
        }
        let enabled = self.pending.is_none() && !self.new_dialog;
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{}{} — Paint Rust",
            name,
            if self.document.dirty() { " *" } else { "" }
        )));
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            if !enabled {
                ui.disable();
            }
            ui.horizontal(|ui| {
                if ui.button("New").on_hover_text("Ctrl+N").clicked() {
                    self.request(Action::New, ctx);
                }
                if ui.button("Open…").on_hover_text("Ctrl+O").clicked() {
                    self.request(Action::Open, ctx);
                }
                if ui.button("Save").on_hover_text("Ctrl+S").clicked() {
                    self.save(false);
                }
                if ui.button("Save as…").clicked() {
                    self.save(true);
                }
                ui.separator();
                if ui
                    .add_enabled(self.document.can_undo(), egui::Button::new("Undo"))
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(self.document.can_redo(), egui::Button::new("Redo"))
                    .clicked()
                {
                    self.redo();
                }
                if ui.button("Invert colors").clicked() {
                    self.finish_stroke();
                    self.document.invert();
                    self.texture_dirty = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Brush size");
                ui.add(egui::Slider::new(&mut self.brush_size, 1.0..=128.0).suffix(" px"));
                ui.separator();
                ui.label("Zoom");
                ui.add(
                    egui::Slider::new(&mut self.zoom, 0.1..=4.0)
                        .logarithmic(true)
                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                );
                if ui.button("100%").clicked() {
                    self.zoom = 1.0;
                }
                if ui.button("Fit").clicked() {
                    let available = ctx.content_rect().size() - Vec2::new(250.0, 150.0);
                    self.zoom = (available.x / self.document.width as f32)
                        .min(available.y / self.document.height as f32)
                        .clamp(0.1, 4.0);
                }
            });
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} × {} px  |  RGBA  |  {:.0}%",
                    self.document.width,
                    self.document.height,
                    self.zoom * 100.0
                ));
                ui.separator();
                ui.label(&self.status);
            });
        });
        egui::SidePanel::left("tools")
            .exact_width(180.0)
            .resizable(false)
            .show(ctx, |ui| {
                if !enabled || self.document.editing() {
                    ui.disable();
                }
                ui.heading("Tools");
                for (tool, label) in [
                    (Tool::Brush, "Brush                 B"),
                    (Tool::Eraser, "Eraser                E"),
                    (Tool::Fill, "Flood fill           F"),
                    (Tool::Picker, "Color picker      I"),
                ] {
                    ui.selectable_value(&mut self.tool, tool, label);
                }
                ui.separator();
                ui.heading("Color");
                ui.color_edit_button_srgba(&mut self.color);
                ui.horizontal_wrapped(|ui| {
                    for color in [
                        Color32::BLACK,
                        Color32::WHITE,
                        Color32::from_rgb(225, 65, 75),
                        Color32::from_rgb(245, 170, 40),
                        Color32::from_rgb(70, 175, 115),
                        Color32::from_rgb(42, 113, 224),
                        Color32::from_rgb(153, 90, 210),
                    ] {
                        if ui.add(egui::Button::new("  ").fill(color)).clicked() {
                            self.color = color;
                        }
                    }
                });
                ui.label(format!(
                    "#{:02X}{:02X}{:02X}",
                    self.color.r(),
                    self.color.g(),
                    self.color.b()
                ));
                ui.separator();
                ui.heading("History");
                ui.small(format!(
                    "{:.1} / 128 MiB",
                    self.document.history_bytes() as f32 / (1024.0 * 1024.0)
                ));
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .show(ui, |ui| {
                        for label in self.document.history() {
                            ui.label(label);
                        }
                    });
                ui.separator();
                ui.small(
                    "Drag to paint.\nEscape cancels a stroke.\nScroll to navigate the canvas.",
                );
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(&ctx.style())
                    .fill(crate::appearance::workspace_color(ctx.theme())),
            )
            .show(ctx, |ui| {
                if !enabled {
                    ui.disable();
                }
                self.canvas(ui);
            });
        self.dialogs(ctx);
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.suppress_stroke = false;
        }
        if self.texture_dirty {
            ctx.request_repaint();
        }
    }
}

impl eframe::App for Editor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(editor: &mut Editor, ctx: &egui::Context, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 800.0))),
            events,
            focused: true,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| editor.show(ctx));
    }

    fn pointer(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn pointer_stroke_commits_once_and_undo_restores_canvas() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        frame(&mut editor, &ctx, vec![]);
        let start = Pos2::new(350.0, 200.0);
        let end = Pos2::new(450.0, 230.0);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
        );
        frame(&mut editor, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut editor, &ctx, vec![pointer(end, false)]);
        assert!(editor.document.dirty());
        assert_eq!(editor.document.history().count(), 1);
        assert!(editor.document.pixels().iter().any(|p| *p != [255; 4]));
        editor.undo();
        assert!(editor.document.pixels().iter().all(|p| *p == [255; 4]));
    }

    #[test]
    fn fast_click_paints_a_dot_and_commits() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        frame(&mut editor, &ctx, vec![]);
        let point = Pos2::new(350.0, 200.0);
        frame(
            &mut editor,
            &ctx,
            vec![
                egui::Event::PointerMoved(point),
                pointer(point, true),
                pointer(point, false),
            ],
        );
        assert!(editor.document.dirty());
        assert_eq!(editor.document.history().count(), 1);
        assert!(!editor.document.editing());
    }

    #[test]
    fn destructive_action_waits_for_unsaved_changes_decision() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document.invert();
        editor.request(Action::New, &ctx);
        assert!(editor.pending.is_some());
        assert!(!editor.new_dialog);
        assert!(editor.document.dirty());
    }

    #[test]
    fn escape_cancels_stroke_without_restarting_while_button_is_held() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        frame(&mut editor, &ctx, vec![]);
        let point = Pos2::new(350.0, 200.0);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(point), pointer(point, true)],
        );
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(Pos2::new(400.0, 200.0))],
        );
        frame(&mut editor, &ctx, vec![pointer(point, false)]);
        assert!(!editor.document.dirty());
        assert!(!editor.document.can_undo());
    }
}
