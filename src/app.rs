use crate::icons::Icon;
use eframe::egui::{self, Color32, Key, Pos2, Rect, Sense, TextureHandle, TextureOptions, Vec2};
use paint_rust::effects::Effect;
use paint_rust::transform::Orientation;
use paint_rust::{
    document::{BlendMode, Document, GradientStyle, Selection, Shape, ShapeStyle},
    files,
};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Brush,
    Eraser,
    Fill,
    Picker,
    Line,
    Rectangle,
    Ellipse,
    Select,
    Text,
    Move,
    Gradient,
}

#[derive(Clone, Copy)]
enum Action {
    New,
    Open,
    Close,
    Recovery,
}

struct ResizeDialog {
    width: u32,
    height: u32,
    canvas: bool,
    lock_ratio: bool,
    ratio: f64,
    smooth: bool,
    anchor: usize,
}

struct TextDialog {
    content: String,
    font: ab_glyph::FontArc,
    origin: (f32, f32),
    size: f32,
    color: Color32,
    focus: bool,
}

struct EffectDialog {
    effect: Effect,
    preview: crate::preview::EffectPreview,
}

pub struct Editor {
    document: Document,
    texture: Option<TextureHandle>,
    texture_dirty: bool,
    tool: Tool,
    color: Color32,
    secondary_color: Color32,
    radial_gradient: bool,
    brush_size: f32,
    shape_filled: bool,
    move_selection_only: bool,
    shape_origin: Option<(f32, f32)>,
    selection_before: Option<Option<Selection>>,
    zoom: f32,
    last_point: Option<(f32, f32)>,
    path: Option<PathBuf>,
    status: String,
    pending: Option<Action>,
    new_dialog: bool,
    layer_properties: Option<(String, u8, BlendMode)>,
    resize_dialog: Option<ResizeDialog>,
    text_dialog: Option<TextDialog>,
    effect_dialog: Option<EffectDialog>,
    new_width: u32,
    new_height: u32,
    transparent: bool,
    allow_close: bool,
    suppress_stroke: bool,
    appearance: Option<std::sync::mpsc::Receiver<egui::Theme>>,
    clipboard: Option<Box<dyn crate::clipboard::Backend>>,
    recovery: Option<paint_rust::recovery::Recovery>,
    recoverable: Vec<paint_rust::recovery::Candidate>,
    recovery_prompt: bool,
}

impl Editor {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut editor = Self::blank(&cc.egui_ctx);
        editor.appearance = Some(crate::appearance::connect(&cc.egui_ctx));
        if let Some(path) = std::env::args_os().nth(1) {
            editor.load(PathBuf::from(path));
        }
        match paint_rust::recovery::Recovery::default_root().and_then(|root| {
            let recovery = paint_rust::recovery::Recovery::new(&root)?;
            let candidates = paint_rust::recovery::Recovery::discover(&root)?;
            Ok((recovery, candidates))
        }) {
            Ok((recovery, candidates)) => {
                editor.recovery = Some(recovery);
                editor.recoverable = candidates;
                editor.recovery_prompt =
                    std::env::args_os().len() == 1 && !editor.recoverable.is_empty();
            }
            Err(error) => editor.status = format!("Recovery unavailable: {error}"),
        }
        editor
    }

    fn blank(ctx: &egui::Context) -> Self {
        crate::appearance::configure(ctx);
        egui_extras::install_image_loaders(ctx);
        Self {
            document: Document::new(960, 640, [255; 4]).unwrap(),
            texture: None,
            texture_dirty: true,
            tool: Tool::Brush,
            color: Color32::from_rgb(42, 113, 224),
            secondary_color: Color32::WHITE,
            radial_gradient: false,
            brush_size: 12.0,
            shape_filled: false,
            move_selection_only: false,
            shape_origin: None,
            selection_before: None,
            zoom: 1.0,
            last_point: None,
            path: None,
            status: "Ready — draw on the canvas to begin".into(),
            pending: None,
            new_dialog: false,
            layer_properties: None,
            resize_dialog: None,
            text_dialog: None,
            effect_dialog: None,
            new_width: 960,
            new_height: 640,
            transparent: false,
            allow_close: false,
            suppress_stroke: false,
            appearance: None,
            clipboard: None,
            recovery: None,
            recoverable: Vec::new(),
            recovery_prompt: false,
        }
    }

    fn clear_recovery(&mut self) {
        if let Some(recovery) = &mut self.recovery
            && let Err(error) = recovery.clear()
        {
            self.status = format!("Could not clear the recovery copy: {error}");
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
                self.clear_recovery();
            }
            Err(error) => self.status = format!("Could not open image: {error}"),
        }
    }

    fn save(&mut self, save_as: bool) -> bool {
        self.finish_stroke();
        let existing = self.path.as_ref().filter(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("prust"))
        });
        let path = if !save_as && existing.is_some() {
            existing.cloned()
        } else {
            rfd::FileDialog::new()
                .add_filter("Paint Rust project", &["prust"])
                .set_file_name(
                    self.path
                        .as_ref()
                        .and_then(|p| p.file_stem())
                        .and_then(|s| s.to_str())
                        .map(|s| format!("{s}.prust"))
                        .unwrap_or_else(|| "Untitled.prust".into()),
                )
                .save_file()
                .map(|p| {
                    if p.extension().is_none() {
                        p.with_extension("prust")
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
            .is_some_and(|e| e.eq_ignore_ascii_case("prust"))
        {
            self.status = "Could not save project: choose a filename ending in .prust".into();
            return false;
        }
        match files::save_project(&self.document, &path) {
            Ok(()) => {
                self.document.mark_saved();
                self.status = format!("Saved {}", path.display());
                self.path = Some(path);
                self.clear_recovery();
                true
            }
            Err(error) => {
                self.status = format!("Could not save project: {error}");
                false
            }
        }
    }

    fn export_png(&mut self) {
        self.finish_stroke();
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG image", &["png"])
            .set_file_name("Untitled.png")
            .save_file()
        else {
            return;
        };
        let path = if path.extension().is_none() {
            path.with_extension("png")
        } else {
            path
        };
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("png"))
        {
            self.status = "Could not export image: choose a filename ending in .png".into();
            return;
        }
        self.status = match files::save_png(&self.document, &path) {
            Ok(()) => format!(
                "Exported {} — save the project to preserve editable layers",
                path.display()
            ),
            Err(error) => format!("Could not export image: {error}"),
        };
    }

    fn import_layer(&mut self) {
        self.finish_stroke();
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import image as layer")
            .add_filter(
                "Images and projects",
                &["png", "jpg", "jpeg", "bmp", "webp", "prust"],
            )
            .pick_file()
        else {
            return;
        };
        self.import_layer_from_path(&path);
    }

    fn import_layer_from_path(&mut self, path: &std::path::Path) {
        let result = files::open(path).and_then(|image| {
            let name = path.file_stem().unwrap_or_default().to_string_lossy();
            self.document
                .import_layer(image.width, image.height, &image.composite(), &name)?;
            Ok((image.width, image.height))
        });
        match result {
            Ok((width, height)) => {
                self.texture_dirty = true;
                let clipped = self
                    .document
                    .selection()
                    .is_some_and(|s| s.width < width || s.height < height);
                self.status = if clipped {
                    "Imported image as a new layer; image clipped to the canvas".into()
                } else {
                    "Imported image as a new layer".into()
                };
            }
            Err(error) => self.status = format!("Could not import image: {error}"),
        }
    }

    fn begin_text(&mut self, ctx: &egui::Context, origin: (f32, f32)) {
        let data = ctx.fonts(|fonts| {
            let definitions = fonts.definitions();
            definitions
                .families
                .get(&egui::FontFamily::Proportional)
                .and_then(|names| names.first())
                .and_then(|name| definitions.font_data.get(name))
                .cloned()
        });
        let font = data
            .and_then(|data| {
                ab_glyph::FontVec::try_from_vec_and_index(data.font.to_vec(), data.index).ok()
            })
            .map(ab_glyph::FontArc::new);
        if let Some(font) = font {
            self.document.begin("Text");
            self.text_dialog = Some(TextDialog {
                content: String::new(),
                font,
                origin,
                size: 32.0,
                color: self.color,
                focus: true,
            });
        } else {
            self.status = "Could not load a font for the text tool".into();
        }
    }

    fn clipboard(&mut self) -> Result<&mut (dyn crate::clipboard::Backend + 'static), String> {
        if self.clipboard.is_none() {
            self.clipboard = Some(Box::new(crate::clipboard::Clipboard::connect()?));
        }
        Ok(self.clipboard.as_deref_mut().expect("clipboard connected"))
    }
    fn copy(&mut self, cut: bool, merged: bool) {
        self.finish_stroke();
        let (width, height, pixels) = self.document.copy_pixels(merged);
        match self
            .clipboard()
            .and_then(|c| c.write(width, height, &pixels))
        {
            Ok(()) => {
                if cut {
                    self.document.clear_pixels();
                    self.texture_dirty = true;
                }
                self.status = if cut {
                    "Cut pixels to clipboard".into()
                } else {
                    "Copied image to clipboard".into()
                };
            }
            Err(error) => self.status = format!("Could not copy image: {error}"),
        }
    }
    fn paste(&mut self) {
        self.finish_stroke();
        match self.clipboard().and_then(|c| c.read()) {
            Ok((width, height, pixels)) => {
                match self.document.paste_layer(width, height, &pixels) {
                    Ok(_) => {
                        self.texture_dirty = true;
                        self.status = if self
                            .document
                            .selection()
                            .is_some_and(|s| s.width < width || s.height < height)
                        {
                            "Pasted on a new layer; image clipped to the canvas".into()
                        } else {
                            "Pasted on a new layer".into()
                        };
                    }
                    Err(error) => self.status = format!("Could not paste image: {error}"),
                }
            }
            Err(error) => {
                self.status = format!("Could not read an image from the clipboard: {error}")
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
            Action::Recovery => self.recovery_prompt = true,
            Action::Open => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(
                        "Projects and images",
                        &["prust", "png", "jpg", "jpeg", "bmp", "webp"],
                    )
                    .pick_file()
                {
                    self.load(path);
                }
            }
            Action::Close => {
                self.clear_recovery();
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn finish_stroke(&mut self) {
        if self.text_dialog.is_some() || self.effect_dialog.is_some() {
            return;
        }
        if self.document.editing() {
            self.document.commit();
            self.texture_dirty = true;
        }
        self.last_point = None;
        self.shape_origin = None;
        self.selection_before = None;
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
            if i.events.iter().any(|event| matches!(event,egui::Event::Copy)) { Some(11)
            } else if i.events.iter().any(|event| matches!(event,egui::Event::Cut)) { Some(12)
            // The window backend consumes Ctrl+V presses when no text is available.
            // Its release event remains available for image clipboard handling.
            } else if i.events.iter().any(|event| matches!(event,egui::Event::Key{key:Key::V,pressed:false,modifiers,..} if modifiers.command)) { Some(13)
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::N) {
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
            } else if i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::A) {
                Some(8)
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::A) {
                Some(7)
            } else if i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::X) {
                Some(10)
            } else if i.key_pressed(Key::Delete) {
                Some(9)
            } else if i.key_pressed(Key::Escape) {
                Some(6)
            } else if !i.modifiers.command && !self.document.editing() && self.last_point.is_none()
            {
                if i.key_pressed(Key::B) {
                    self.tool = Tool::Brush;
                }
                if i.key_pressed(Key::E) {
                    self.tool = Tool::Eraser;
                }
                if i.key_pressed(Key::F) {
                    self.tool = Tool::Fill;
                }
                if i.key_pressed(Key::G) { self.tool=Tool::Gradient; }
                if i.key_pressed(Key::X) { std::mem::swap(&mut self.color,&mut self.secondary_color); }
                if i.key_pressed(Key::M) { self.tool=Tool::Move; }
                if i.key_pressed(Key::T) { self.tool = Tool::Text; }
                if i.key_pressed(Key::S) {
                    self.tool = Tool::Select;
                }
                if i.key_pressed(Key::L) {
                    self.tool = Tool::Line;
                }
                if i.key_pressed(Key::R) {
                    self.tool = Tool::Rectangle;
                }
                if i.key_pressed(Key::O) {
                    self.tool = Tool::Ellipse;
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
                self.shape_origin = None;
                if let Some(previous) = self.selection_before.take() {
                    self.document.set_selection(previous);
                }
                self.texture_dirty = true;
            }
            Some(7) => {
                self.finish_stroke();
                self.document.set_selection(Some(Selection {
                    x: 0,
                    y: 0,
                    width: self.document.width,
                    height: self.document.height,
                }));
            }
            Some(8) => {
                self.finish_stroke();
                self.document.set_selection(None);
            }
            Some(9) => {
                self.finish_stroke();
                self.document.clear_pixels();
                self.texture_dirty = true;
            }
            Some(10) => {
                self.finish_stroke();
                self.document.crop_to_selection();
                self.texture_dirty = true;
            }
            Some(11) => self.copy(false, false),
            Some(12) => self.copy(true, false),
            Some(13) => self.paste(),
            _ => {}
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        if self.texture_dirty {
            let bytes: Vec<u8> = self
                .document
                .composite()
                .iter()
                .flatten()
                .copied()
                .collect();
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
                if let Some(selection) = self.document.selection() {
                    let outline = Rect::from_min_size(
                        rect.min + Vec2::new(selection.x as f32, selection.y as f32) * self.zoom,
                        Vec2::new(selection.width as f32, selection.height as f32) * self.zoom,
                    );
                    painter.rect_stroke(
                        outline,
                        0.0,
                        egui::Stroke::new(3.0_f32, Color32::BLACK),
                        egui::StrokeKind::Inside,
                    );
                    painter.rect_stroke(
                        outline,
                        0.0,
                        egui::Stroke::new(1.0_f32, Color32::WHITE),
                        egui::StrokeKind::Inside,
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
                    && ui.input(|i| i.focused)
                    && !self.suppress_stroke
                    && ((response.is_pointer_button_down_on()
                        && ui.input(|i| i.pointer.primary_down()))
                        || response.clicked_by(egui::PointerButton::Primary)
                        || (response.drag_stopped_by(egui::PointerButton::Primary)
                            && self.last_point.is_some()))
                    && let Some(pos) = response
                        .interact_pointer_pos()
                        .or_else(|| ui.input(|i| i.pointer.latest_pos()))
                {
                    let local = (pos - rect.min) / self.zoom;
                    let point = (local.x, local.y);
                    if self.last_point.is_none() {
                        let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
                        let origin = (origin - rect.min) / self.zoom;
                        self.shape_origin = Some((origin.x, origin.y));
                        match self.tool {
                            Tool::Gradient => self.document.begin("Gradient"),
                            Tool::Move => self.document.begin(if self.move_selection_only {
                                "Move selection"
                            } else {
                                "Move pixels"
                            }),
                            Tool::Text => self.begin_text(ui.ctx(), point),
                            Tool::Select => {
                                self.selection_before = Some(self.document.selection());
                            }
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
                            Tool::Line => self.document.begin("Line"),
                            Tool::Rectangle => self.document.begin("Rectangle"),
                            Tool::Ellipse => self.document.begin("Ellipse"),
                            _ => self.document.begin(if self.tool == Tool::Eraser {
                                "Eraser"
                            } else {
                                "Brush stroke"
                            }),
                        }
                    }
                    if matches!(self.tool, Tool::Brush | Tool::Eraser) {
                        self.document.stroke(
                            self.last_point.or(self.shape_origin).unwrap_or(point),
                            point,
                            self.brush_size,
                            self.color.to_srgba_unmultiplied(),
                            self.tool == Tool::Eraser,
                        );
                        self.texture_dirty = true;
                    }
                    let shape = match self.tool {
                        Tool::Line => Some(Shape::Line),
                        Tool::Rectangle => Some(Shape::Rectangle),
                        Tool::Ellipse => Some(Shape::Ellipse),
                        _ => None,
                    };
                    if let Some(shape) = shape {
                        let origin = self.shape_origin.unwrap_or(point);
                        let mut end = point;
                        if ui.input(|i| i.modifiers.shift) {
                            let dx = point.0 - origin.0;
                            let dy = point.1 - origin.1;
                            if shape == Shape::Line {
                                let length = dx.hypot(dy);
                                let angle = (dy.atan2(dx) / std::f32::consts::FRAC_PI_4).round()
                                    * std::f32::consts::FRAC_PI_4;
                                end = (
                                    origin.0 + length * angle.cos(),
                                    origin.1 + length * angle.sin(),
                                );
                            } else {
                                let size = dx.abs().max(dy.abs());
                                end =
                                    (origin.0 + size * dx.signum(), origin.1 + size * dy.signum());
                            }
                        }
                        self.document.preview_shape(
                            shape,
                            origin,
                            end,
                            ShapeStyle {
                                color: self.color.to_srgba_unmultiplied(),
                                width: self.brush_size,
                                filled: self.shape_filled,
                            },
                        );
                        self.texture_dirty = true;
                    }
                    if self.tool == Tool::Gradient {
                        self.document.preview_gradient(
                            self.shape_origin.unwrap_or(point),
                            point,
                            GradientStyle {
                                start: self.color.to_srgba_unmultiplied(),
                                end: self.secondary_color.to_srgba_unmultiplied(),
                                radial: self.radial_gradient,
                            },
                        );
                        self.texture_dirty = true;
                    }
                    if self.tool == Tool::Move {
                        let origin = self.shape_origin.unwrap_or(point);
                        let mut dx = (point.0 - origin.0).round() as i32;
                        let mut dy = (point.1 - origin.1).round() as i32;
                        if ui.input(|i| i.modifiers.shift) {
                            if dx.abs() >= dy.abs() {
                                dy = 0;
                            } else {
                                dx = 0;
                            }
                        }
                        self.document.preview_move(
                            dx,
                            dy,
                            ui.input(|i| i.modifiers.command),
                            self.move_selection_only,
                        );
                        self.texture_dirty = true;
                    }
                    if self.tool == Tool::Select {
                        let origin = self.shape_origin.unwrap_or(point);
                        let x = origin
                            .0
                            .min(point.0)
                            .floor()
                            .clamp(0.0, self.document.width as f32)
                            as u32;
                        let y = origin
                            .1
                            .min(point.1)
                            .floor()
                            .clamp(0.0, self.document.height as f32)
                            as u32;
                        let right = origin
                            .0
                            .max(point.0)
                            .ceil()
                            .clamp(0.0, self.document.width as f32)
                            as u32;
                        let bottom = origin
                            .1
                            .max(point.1)
                            .ceil()
                            .clamp(0.0, self.document.height as f32)
                            as u32;
                        self.document.set_selection(Some(Selection {
                            x,
                            y,
                            width: right - x,
                            height: bottom - y,
                        }));
                        ui.ctx().request_repaint();
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
                        if ui.add(Icon::Save.button("Save")).clicked() && self.save(false) {
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
        if let Some((mut name, mut opacity, mut mode)) = self.layer_properties.take() {
            let mut keep = true;
            egui::Window::new("Layer properties")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut name);
                    ui.add(egui::Slider::new(&mut opacity, 0..=255).text("Opacity"));
                    egui::ComboBox::from_label("Blend mode")
                        .selected_text(mode.name())
                        .show_ui(ui, |ui| {
                            for candidate in BlendMode::ALL {
                                ui.selectable_value(&mut mode, candidate, candidate.name());
                            }
                        });
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(name.len() <= 4096, egui::Button::new("Apply"))
                            .clicked()
                        {
                            self.document.set_layer_style(name.clone(), opacity, mode);
                            self.texture_dirty = true;
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                });
            if keep {
                self.layer_properties = Some((name, opacity, mode));
            }
        }
        if let Some(mut dialog) = self.resize_dialog.take() {
            let mut keep = true;
            egui::Window::new(if dialog.canvas {
                "Canvas size"
            } else {
                "Resize image"
            })
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Width");
                    if ui
                        .add(
                            egui::DragValue::new(&mut dialog.width)
                                .range(1..=16384)
                                .suffix(" px"),
                        )
                        .changed()
                        && dialog.lock_ratio
                    {
                        dialog.height = (f64::from(dialog.width) / dialog.ratio)
                            .round()
                            .clamp(1.0, 16384.0) as u32;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Height");
                    if ui
                        .add(
                            egui::DragValue::new(&mut dialog.height)
                                .range(1..=16384)
                                .suffix(" px"),
                        )
                        .changed()
                        && dialog.lock_ratio
                    {
                        dialog.width = (f64::from(dialog.height) * dialog.ratio)
                            .round()
                            .clamp(1.0, 16384.0) as u32;
                    }
                });
                ui.checkbox(&mut dialog.lock_ratio, "Keep aspect ratio");
                if dialog.canvas {
                    let anchors = [
                        "Top left",
                        "Top center",
                        "Top right",
                        "Middle left",
                        "Center",
                        "Middle right",
                        "Bottom left",
                        "Bottom center",
                        "Bottom right",
                    ];
                    egui::ComboBox::from_label("Anchor")
                        .selected_text(anchors[dialog.anchor])
                        .show_ui(ui, |ui| {
                            for (index, name) in anchors.iter().enumerate() {
                                ui.selectable_value(&mut dialog.anchor, index, *name);
                            }
                        });
                    ui.label("New canvas area is transparent on every layer.");
                } else {
                    ui.checkbox(&mut dialog.smooth, "Smooth resampling (bilinear)");
                }
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        let result = if dialog.canvas {
                            self.document
                                .resize_canvas(dialog.width, dialog.height, dialog.anchor)
                        } else {
                            self.document
                                .resize_image(dialog.width, dialog.height, dialog.smooth)
                        };
                        match result {
                            Ok(_) => {
                                self.texture_dirty = true;
                                self.status = "Image dimensions updated".into();
                                keep = false;
                            }
                            Err(error) => self.status = format!("Could not resize: {error}"),
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        keep = false;
                    }
                });
                if self.status.starts_with("Could not resize") {
                    ui.colored_label(Color32::LIGHT_RED, &self.status);
                }
            });
            if keep {
                self.resize_dialog = Some(dialog);
            }
        }
        if let Some(mut dialog) = self.text_dialog.take() {
            let mut keep = true;
            let cancel = ctx.input(|i| i.key_pressed(Key::Escape));
            if cancel {
                self.document.cancel();
                self.texture_dirty = true;
                keep = false;
            } else {
                egui::Window::new("Text")
                    .collapsible(false)
                    .resizable(true)
                    .default_width(320.0)
                    .show(ctx, |ui| {
                        let response = ui.add(
                            egui::TextEdit::multiline(&mut dialog.content)
                                .desired_rows(4)
                                .char_limit(4096),
                        );
                        if dialog.focus {
                            response.request_focus();
                            dialog.focus = false;
                        }
                        let mut changed = response.changed();
                        changed |= ui
                            .add(egui::Slider::new(&mut dialog.size, 4.0..=256.0).text("Size (px)"))
                            .changed();
                        changed |= ui.color_edit_button_srgba(&mut dialog.color).changed();
                        if changed {
                            match self.document.preview_text(paint_rust::text::TextStyle {
                                font: &dialog.font,
                                text: &dialog.content,
                                origin: dialog.origin,
                                size: dialog.size,
                                color: dialog.color.to_srgba_unmultiplied(),
                            }) {
                                Ok(()) => self.texture_dirty = true,
                                Err(error) => self.status = error,
                            }
                        }
                        ui.horizontal(|ui| {
                            if ui.button("Apply").clicked() {
                                self.document.commit();
                                self.texture_dirty = true;
                                keep = false;
                            }
                            if ui.button("Cancel").clicked() {
                                self.document.cancel();
                                self.texture_dirty = true;
                                keep = false;
                            }
                        });
                        ui.small("Text becomes pixels on the active layer when applied.");
                    });
            }
            if keep {
                self.text_dialog = Some(dialog);
            } else {
                self.last_point = None;
                self.shape_origin = None;
            }
        }
        if let Some(mut dialog) = self.effect_dialog.take() {
            let mut keep = true;
            if let Some(pixels) = dialog.preview.poll() {
                match self.document.publish_preview(pixels) {
                    Ok(()) => self.texture_dirty = true,
                    Err(error) => self.status = error,
                }
            }
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.document.cancel();
                self.texture_dirty = true;
                keep = false;
            } else {
                egui::Window::new(dialog.effect.name())
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        let mut changed = false;
                        match &mut dialog.effect {
                            Effect::BrightnessContrast {
                                brightness,
                                contrast,
                            } => {
                                changed |= ui
                                    .add(
                                        egui::Slider::new(brightness, -100..=100)
                                            .text("Brightness"),
                                    )
                                    .changed();
                                changed |= ui
                                    .add(egui::Slider::new(contrast, -100..=100).text("Contrast"))
                                    .changed();
                            }
                            Effect::Saturation(amount) => {
                                changed |= ui
                                    .add(egui::Slider::new(amount, 0.0..=2.0).text("Saturation"))
                                    .changed();
                            }
                            Effect::BoxBlur(radius) => {
                                changed |= ui
                                    .add(egui::Slider::new(radius, 0..=32).text("Radius (px)"))
                                    .changed();
                            }
                            _ => {
                                ui.label("Preview on the active layer");
                            }
                        }
                        if changed {
                            dialog.preview.request(dialog.effect);
                        }
                        if !dialog.preview.ready {
                            ui.label("Rendering preview…");
                        }
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(dialog.preview.ready, egui::Button::new("Apply"))
                                .clicked()
                            {
                                self.document.commit();
                                self.texture_dirty = true;
                                keep = false;
                            }
                            if ui.button("Cancel").clicked() {
                                self.document.cancel();
                                self.texture_dirty = true;
                                keep = false;
                            }
                        });
                    });
            }
            if keep {
                self.effect_dialog = Some(dialog);
            }
        }
        if self.recovery_prompt {
            let mut chosen = None;
            egui::Window::new("Recover unsaved work")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("An earlier session left unsaved image data.");
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            for (index, candidate) in self.recoverable.iter().enumerate() {
                                let minutes =
                                    candidate.modified.elapsed().unwrap_or_default().as_secs() / 60;
                                ui.horizontal(|ui| {
                                    ui.label(format!(
                                        "Session {} — last copied {minutes} min ago",
                                        index + 1
                                    ));
                                    if ui.button("Recover").clicked() {
                                        chosen = Some((index, true));
                                    }
                                    if ui.button("Discard").clicked() {
                                        chosen = Some((index, false));
                                    }
                                });
                            }
                        });
                    if ui.button("Later").clicked() {
                        self.recovery_prompt = false;
                    }
                });
            if let Some((index, recover)) = chosen {
                if recover {
                    match files::open(&self.recoverable[index].path) {
                        Ok(mut document) => {
                            document.mark_unsaved();
                            self.clear_recovery();
                            self.document = document;
                            self.path = None;
                            self.texture_dirty = true;
                            self.zoom = 1.0;
                            let candidate = self.recoverable.remove(index);
                            if let Some(recovery) = &mut self.recovery {
                                recovery.adopt(candidate);
                            }
                            self.status = "Recovered unsaved work — save it as a project".into();
                            self.recovery_prompt = false;
                        }
                        Err(error) => {
                            self.status = format!("Could not recover the project: {error}")
                        }
                    }
                } else {
                    let candidate = self.recoverable.remove(index);
                    if let Err(error) = candidate.discard() {
                        self.status = format!("Could not discard recovery data: {error}");
                    }
                    if self.recoverable.is_empty() {
                        self.recovery_prompt = false;
                    }
                }
            }
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
                                    self.clear_recovery();
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
        if ctx.input(|i| i.viewport().close_requested()) && self.effect_dialog.is_some() {
            self.effect_dialog = None;
            self.document.cancel();
            self.texture_dirty = true;
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.text_dialog.is_some() {
            self.text_dialog = None;
            self.document.commit();
            self.texture_dirty = true;
        }
        if let Some(receiver) = &self.appearance
            && let Some(theme) = receiver.try_iter().last()
        {
            ctx.set_theme(theme);
            ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(match theme {
                egui::Theme::Light => egui::SystemTheme::Light,
                egui::Theme::Dark => egui::SystemTheme::Dark,
            }));
        }
        if !ctx.input(|i| i.focused) {
            self.finish_stroke();
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && self.document.dirty()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Action::Close);
        }
        let enabled = self.pending.is_none()
            && !self.new_dialog
            && self.layer_properties.is_none()
            && self.resize_dialog.is_none()
            && self.text_dialog.is_none()
            && self.effect_dialog.is_none()
            && !self.recovery_prompt;
        if enabled {
            self.shortcuts(ctx);
        }
        let enabled = self.pending.is_none()
            && !self.new_dialog
            && self.layer_properties.is_none()
            && self.resize_dialog.is_none()
            && self.text_dialog.is_none()
            && self.effect_dialog.is_none()
            && !self.recovery_prompt;
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
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(Icon::New.button("New"))
                    .on_hover_text("Ctrl+N")
                    .clicked()
                {
                    self.request(Action::New, ctx);
                }
                if ui
                    .add(Icon::Open.button("Open…"))
                    .on_hover_text("Ctrl+O")
                    .clicked()
                {
                    self.request(Action::Open, ctx);
                }
                if ui
                    .add(Icon::Save.button("Save"))
                    .on_hover_text("Ctrl+S")
                    .clicked()
                {
                    self.save(false);
                }
                if ui.add(Icon::SaveAs.button("Save as…")).clicked() {
                    self.save(true);
                }
                if ui.add(Icon::SaveAs.button("Export PNG…")).clicked() {
                    self.export_png();
                }
                ui.separator();
                if ui
                    .add_enabled(self.document.can_undo(), Icon::Undo.button("Undo"))
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(self.document.can_redo(), Icon::Redo.button("Redo"))
                    .clicked()
                {
                    self.redo();
                }
                if !self.recoverable.is_empty() && ui.button("Recover…").clicked() {
                    self.request(Action::Recovery, ctx);
                }
                ui.menu_button("Edit", |ui| {
                    if ui.add(Icon::Copy.button("Copy    Ctrl+C")).clicked() {
                        self.copy(false, false);
                        ui.close();
                    }
                    if ui.add(Icon::Copy.button("Copy merged")).clicked() {
                        self.copy(false, true);
                        ui.close();
                    }
                    if ui.add(Icon::Cut.button("Cut    Ctrl+X")).clicked() {
                        self.copy(true, false);
                        ui.close();
                    }
                    if ui
                        .add(Icon::Paste.button("Paste as layer    Ctrl+V"))
                        .clicked()
                    {
                        self.paste();
                        ui.close();
                    }
                });
                ui.menu_button("Image", |ui| {
                    for (label, canvas) in [("Resize image…", false), ("Canvas size…", true)] {
                        if ui.button(label).clicked() {
                            self.finish_stroke();
                            self.resize_dialog = Some(ResizeDialog {
                                width: self.document.width,
                                height: self.document.height,
                                canvas,
                                lock_ratio: !canvas,
                                ratio: f64::from(self.document.width)
                                    / f64::from(self.document.height),
                                smooth: true,
                                anchor: 4,
                            });
                            ui.close();
                        }
                    }
                    ui.separator();
                    for (label, operation) in [
                        ("Rotate 90° clockwise", Orientation::Clockwise),
                        ("Rotate 90° counterclockwise", Orientation::Counterclockwise),
                        ("Rotate 180°", Orientation::HalfTurn),
                        ("Flip horizontal", Orientation::FlipHorizontal),
                        ("Flip vertical", Orientation::FlipVertical),
                    ] {
                        if ui.button(label).clicked() {
                            self.finish_stroke();
                            match self.document.reorient(operation) {
                                Ok(_) => self.texture_dirty = true,
                                Err(error) => self.status = error,
                            }
                            ui.close();
                        }
                    }
                });
                ui.menu_button("Selection", |ui| {
                    if ui.button("Select all    Ctrl+A").clicked() {
                        self.finish_stroke();
                        self.document.set_selection(Some(Selection {
                            x: 0,
                            y: 0,
                            width: self.document.width,
                            height: self.document.height,
                        }));
                        ui.close();
                    }
                    if ui.button("Deselect    Ctrl+Shift+A").clicked() {
                        self.finish_stroke();
                        self.document.set_selection(None);
                        ui.close();
                    }
                    if ui.button("Delete pixels    Del").clicked() {
                        self.finish_stroke();
                        self.document.clear_pixels();
                        self.texture_dirty = true;
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.document.selection().is_some(),
                            Icon::Crop.button("Crop    Ctrl+Shift+X"),
                        )
                        .clicked()
                    {
                        self.finish_stroke();
                        self.document.crop_to_selection();
                        self.texture_dirty = true;
                        ui.close();
                    }
                });
                ui.menu_button("Adjustments", |ui| {
                    for effect in [
                        Effect::BrightnessContrast {
                            brightness: 0,
                            contrast: 0,
                        },
                        Effect::Saturation(1.0),
                        Effect::Grayscale,
                        Effect::Sepia,
                        Effect::BoxBlur(2),
                    ] {
                        if ui.button(effect.name()).clicked() {
                            self.finish_stroke();
                            let preview = crate::preview::EffectPreview::new(
                                ctx.clone(),
                                self.document.shared_pixels(),
                                self.document.width,
                                self.document.height,
                                self.document.selection(),
                                effect,
                            );
                            self.document.begin(effect.name());
                            self.effect_dialog = Some(EffectDialog { effect, preview });
                            ui.close();
                        }
                    }
                });
                if ui.add(Icon::Invert.button("Invert colors")).clicked() {
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
                if ui.add(Icon::ActualSize.button("100%")).clicked() {
                    self.zoom = 1.0;
                }
                if ui.add(Icon::Fit.button("Fit")).clicked() {
                    let available = ctx.content_rect().size() - Vec2::new(470.0, 150.0);
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
                egui::ScrollArea::vertical()
                    .id_salt("tool-panel")
                    .show(ui, |ui| {
                        if !enabled || self.document.editing() {
                            ui.disable();
                        }
                        ui.heading("Tools");
                        for (tool, icon, label, shortcut) in [
                            (Tool::Select, Icon::Select, "Select area", "S"),
                            (Tool::Move, Icon::Move, "Move", "M"),
                            (Tool::Gradient, Icon::Gradient, "Gradient", "G"),
                            (Tool::Text, Icon::Text, "Text", "T"),
                            (Tool::Brush, Icon::Brush, "Brush", "B"),
                            (Tool::Eraser, Icon::Eraser, "Eraser", "E"),
                            (Tool::Fill, Icon::Fill, "Flood fill", "F"),
                            (Tool::Picker, Icon::Picker, "Color picker", "I"),
                            (Tool::Line, Icon::Line, "Line", "L"),
                            (Tool::Rectangle, Icon::Rectangle, "Rectangle", "R"),
                            (Tool::Ellipse, Icon::Ellipse, "Ellipse", "O"),
                        ] {
                            let button = icon
                                .button(label)
                                .selected(self.tool == tool)
                                .shortcut_text(egui::RichText::new(shortcut).color(
                                    if self.tool == tool {
                                        ui.visuals().selection.stroke.color
                                    } else {
                                        ui.visuals().weak_text_color()
                                    },
                                ));
                            if ui.add_sized([ui.available_width(), 34.0], button).clicked() {
                                self.tool = tool;
                            }
                        }
                        if self.tool == Tool::Gradient {
                            ui.checkbox(&mut self.radial_gradient, "Radial gradient");
                        }
                        if self.tool == Tool::Move {
                            ui.checkbox(&mut self.move_selection_only, "Move selection only");
                            ui.small("Ctrl: copy pixels. Shift: constrain direction.");
                        }
                        if matches!(self.tool, Tool::Rectangle | Tool::Ellipse) {
                            ui.checkbox(&mut self.shape_filled, "Filled shape");
                            ui.small("Hold Shift for equal sides.");
                        }
                        if self.tool == Tool::Line {
                            ui.small("Hold Shift for 45° angles.");
                        }
                        ui.separator();
                        ui.heading("Color");
                        ui.horizontal(|ui| {
                            ui.label("Primary");
                            ui.color_edit_button_srgba(&mut self.color);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Secondary");
                            ui.color_edit_button_srgba(&mut self.secondary_color);
                        });
                        if ui.button("Swap colors    X").clicked() {
                            std::mem::swap(&mut self.color, &mut self.secondary_color);
                        }
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
            });
        egui::SidePanel::right("layers")
            .default_width(220.0)
            .width_range(180.0..=320.0)
            .show(ctx, |ui| {
                if !enabled || self.document.editing() {
                    ui.disable();
                }
                ui.heading("Layers");
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(self.document.can_add_layer(), egui::Button::new("Add"))
                        .clicked()
                    {
                        self.document.add_layer(false);
                        self.texture_dirty = true;
                    }
                    if ui
                        .add_enabled(
                            self.document.can_add_layer(),
                            egui::Button::new("Duplicate"),
                        )
                        .clicked()
                    {
                        self.document.add_layer(true);
                        self.texture_dirty = true;
                    }
                    if ui
                        .add_enabled(
                            self.document.layers().len() > 1,
                            egui::Button::new("Delete"),
                        )
                        .clicked()
                    {
                        self.document.remove_layer();
                        self.texture_dirty = true;
                    }
                });
                ui.horizontal(|ui| {
                    let active = self.document.active_layer();
                    if ui
                        .add_enabled(
                            active + 1 < self.document.layers().len(),
                            egui::Button::new("Move up"),
                        )
                        .clicked()
                    {
                        self.document.move_layer(true);
                        self.texture_dirty = true;
                    }
                    if ui
                        .add_enabled(active > 0, egui::Button::new("Move down"))
                        .clicked()
                    {
                        self.document.move_layer(false);
                        self.texture_dirty = true;
                    }
                });
                if ui
                    .add_enabled(
                        self.document.can_add_layer(),
                        egui::Button::new("Import image…"),
                    )
                    .on_hover_text("Add an image or a project's visible image as a new layer")
                    .clicked()
                {
                    self.import_layer();
                }
                if ui
                    .add_enabled(
                        self.document.can_flatten(),
                        egui::Button::new("Flatten image"),
                    )
                    .on_hover_text(
                        "Replace all layers with the visible image. Undo restores the layers.",
                    )
                    .clicked()
                {
                    self.document.flatten();
                    self.texture_dirty = true;
                }
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for index in (0..self.document.layers().len()).rev() {
                            let layer = &self.document.layers()[index];
                            let mut visible = layer.visible;
                            let name = layer.name.clone();
                            ui.horizontal(|ui| {
                                if ui
                                    .checkbox(&mut visible, "")
                                    .on_hover_text("Layer visibility")
                                    .changed()
                                {
                                    self.document.set_visibility(index, visible);
                                    self.texture_dirty = true;
                                }
                                if ui
                                    .selectable_label(index == self.document.active_layer(), name)
                                    .clicked()
                                {
                                    self.document.select_layer(index);
                                }
                            });
                        }
                    });
                ui.separator();
                let layer = &self.document.layers()[self.document.active_layer()];
                ui.label(format!(
                    "Opacity: {:.0}%",
                    f32::from(layer.opacity) * 100.0 / 255.0
                ));
                if ui.button("Layer properties…").clicked() {
                    self.layer_properties =
                        Some((layer.name.clone(), layer.opacity, layer.blend_mode));
                }
                if !layer.visible {
                    ui.small("The active layer is hidden.");
                }
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
        if let Some(recovery) = &mut self.recovery {
            let wake = ctx.clone();
            if let Err(error) = recovery.tick(&self.document, move || wake.request_repaint()) {
                self.status = format!("Could not update the recovery copy: {error}");
            }
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.finish_stroke();
            self.suppress_stroke = false;
        }
        if self.texture_dirty {
            ctx.request_repaint();
        }
    }
}

impl eframe::App for Editor {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.clear_recovery();
    }

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
    fn shape_drag_uses_release_position_and_has_one_undo_step() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.tool = Tool::Rectangle;
        editor.shape_filled = true;
        frame(&mut editor, &ctx, vec![]);
        let start = Pos2::new(350.0, 200.0);
        let middle = Pos2::new(370.0, 220.0);
        let end = Pos2::new(430.0, 280.0);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
        );
        frame(&mut editor, &ctx, vec![egui::Event::PointerMoved(middle)]);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(end), pointer(end, false)],
        );
        assert_eq!(editor.document.history().count(), 1);
        let painted = editor
            .document
            .pixels()
            .iter()
            .filter(|p| **p != [255; 4])
            .count();
        assert!(
            painted >= 75 * 75,
            "release endpoint was not used: {painted}"
        );
        editor.undo();
        assert!(!editor.document.dirty());
    }

    #[test]
    fn selection_gesture_stays_clean_and_escape_restores_previous_selection() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.tool = Tool::Select;
        let previous = Selection {
            x: 5,
            y: 6,
            width: 7,
            height: 8,
        };
        editor.document.set_selection(Some(previous));
        frame(&mut editor, &ctx, vec![]);
        let start = Pos2::new(350.0, 200.0);
        let end = Pos2::new(430.0, 280.0);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
        );
        frame(&mut editor, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_ne!(editor.document.selection(), Some(previous));
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
        frame(&mut editor, &ctx, vec![pointer(end, false)]);
        assert_eq!(editor.document.selection(), Some(previous));
        assert!(!editor.document.dirty());
        assert!(!editor.document.can_undo());
    }

    struct MemoryClipboard {
        image: Option<(u32, u32, Vec<[u8; 4]>)>,
        fail: bool,
    }
    impl crate::clipboard::Backend for MemoryClipboard {
        fn write(&mut self, width: u32, height: u32, pixels: &[[u8; 4]]) -> Result<(), String> {
            if self.fail {
                return Err("Clipboard unavailable".into());
            }
            self.image = Some((width, height, pixels.to_vec()));
            Ok(())
        }
        fn read(&mut self) -> Result<(u32, u32, Vec<[u8; 4]>), String> {
            self.image.clone().ok_or_else(|| "No image".into())
        }
    }

    #[test]
    fn failed_clipboard_write_never_deletes_cut_pixels() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.clipboard = Some(Box::new(MemoryClipboard {
            image: None,
            fail: true,
        }));
        editor.copy(true, false);
        assert!(!editor.document.dirty());
        assert!(!editor.document.can_undo());
        assert!(editor.document.pixels().iter().all(|p| *p == [255; 4]));
    }

    #[test]
    fn native_cut_event_and_paste_release_preserve_image_and_history() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.clipboard = Some(Box::new(MemoryClipboard {
            image: None,
            fail: false,
        }));
        editor.document.set_selection(Some(Selection {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        }));
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![egui::Event::Cut]);
        assert_eq!(editor.document.pixels()[0], [0; 4]);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::Key {
                key: Key::V,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
        );
        assert_eq!(editor.document.layers().len(), 2);
        assert_eq!(editor.document.composite()[0], [255; 4]);
        assert_eq!(editor.document.history().count(), 2);
        editor.undo();
        assert_eq!(editor.document.layers().len(), 1);
        editor.undo();
        assert_eq!(editor.document.pixels()[0], [255; 4]);
    }

    #[test]
    fn text_stays_pending_until_apply_and_escape_restores_pixels() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.tool = Tool::Text;
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
        assert!(editor.text_dialog.is_some());
        assert!(editor.document.editing());
        frame(&mut editor, &ctx, vec![egui::Event::Text("Hello".into())]);
        frame(&mut editor, &ctx, vec![]);
        assert!(editor.document.dirty());
        assert!(!editor.document.can_undo());
        assert!(editor.document.editing());
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
        assert!(editor.text_dialog.is_none());
        assert!(!editor.document.editing());
        assert!(!editor.document.dirty());
        assert!(editor.document.pixels().iter().all(|p| *p == [255; 4]));
    }

    #[test]
    fn effect_preview_is_not_committed_by_idle_frames_and_escape_cancels() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        frame(&mut editor, &ctx, vec![]);
        let effect = Effect::BrightnessContrast {
            brightness: -50,
            contrast: 0,
        };
        let preview = crate::preview::EffectPreview::new(
            ctx.clone(),
            editor.document.shared_pixels(),
            editor.document.width,
            editor.document.height,
            None,
            effect,
        );
        editor.document.begin(effect.name());
        editor.effect_dialog = Some(EffectDialog { effect, preview });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            frame(&mut editor, &ctx, vec![]);
            if editor.effect_dialog.as_ref().unwrap().preview.ready {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(editor.document.dirty());
        assert!(editor.document.editing());
        assert!(!editor.document.can_undo());
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
        assert!(editor.effect_dialog.is_none());
        assert!(!editor.document.dirty());
        assert!(editor.document.pixels().iter().all(|p| *p == [255; 4]));
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
    #[test]
    fn importing_a_project_uses_its_composite_without_replacing_the_destination() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(4, 4, [255; 4]).unwrap();
        editor.path = Some(std::path::PathBuf::from("destination.prust"));
        let mut image = Document::new(2, 2, [200, 50, 80, 123]).unwrap();
        image.add_layer(true);
        image.set_layer_properties("Top".into(), 80);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Zażółć.prust");
        files::save_project(&image, &path).unwrap();
        editor.import_layer_from_path(&path);
        assert_eq!(editor.document.layers().len(), 2);
        assert_eq!(editor.document.layers()[1].name, "Zażółć");
        assert_eq!(editor.document.pixels()[5], image.composite()[0]);
        assert_eq!(
            editor.path.as_deref(),
            Some(std::path::Path::new("destination.prust"))
        );
        assert_eq!((editor.document.width, editor.document.height), (4, 4));
        editor.document.undo();
        assert_eq!(editor.document.layers().len(), 1);
        assert!(!editor.document.dirty());
        editor.document.redo();
        assert_eq!(editor.document.pixels()[5], image.composite()[0]);
        let before = editor.document.layers().to_vec();
        editor.import_layer_from_path(&dir.path().join("missing.png"));
        assert!(editor.document.layers() == before);
        assert!(editor.status.starts_with("Could not import image:"));
    }
}
