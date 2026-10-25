mod about;
mod clone;
mod colors;
mod curves;
mod effect_settings;
mod errors;
mod export;
mod file_operations;
mod help;
mod histogram;
mod lasso;
mod new_image;
mod panels;
mod paste;
mod preferences;
mod recovery_batch;
mod render_controls;
mod rulers;
mod selection_transform;
mod settings;
mod shape_edit;
mod structure_shortcuts;
mod thumbnails;
mod toolbar;
mod transform_handles;
mod workspace;
use crate::icons::Icon;
use colors::Rgba;
use eframe::egui::{self, Color32, Key, Pos2, Rect, Sense, TextureHandle, TextureOptions, Vec2};
use file_operations::{DocumentEdit, FileJob, Outcome};
use paint_rust::effects::Effect;
use paint_rust::selection::CombineMode;
use paint_rust::transform::Orientation;
use paint_rust::{
    document::{BlendMode, Document, GradientStyle, Selection, Shape, ShapeStyle},
    files,
};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Shapes,
    Clone,
    Recolor,
    Lasso,
    ColorSelect,
    Polygon,
    Zoom,
    Pan,
    Brush,
    Pencil,
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
    SaveAll,
    New,
    Open,
    Close,
    CloseDocument,
    CloseAll,
    Recovery,
}

struct ResizeDialog {
    width: u32,
    height: u32,
    canvas: bool,
    lock_ratio: bool,
    ratio: f64,
    resampling: paint_rust::resample::Options,
    units: paint_rust::resolution::Unit,
    dpi: paint_rust::resolution::Dpi,
    link_resolution: bool,
    anchor: usize,
    fill: usize,
    percentage: bool,
    percent: f64,
    original_size: (u32, u32),
}

struct TextDialog {
    font_name: String,
    layout: paint_rust::text::TextLayout,
    preview: crate::preview::EffectPreview,
    content: String,
    font: ab_glyph::FontArc,
    origin: (f32, f32),
    size: f32,
    color: Rgba,
    focus: bool,
}

struct EffectDialog {
    output_histogram: Option<histogram::Histogram>,
    histogram: Option<histogram::Histogram>,
    effect: Effect,
    preview: crate::preview::EffectPreview,
}

pub struct Editor {
    help_open: bool,
    errors: errors::ErrorLog,
    about: about::About,
    preferences: preferences::Preferences,
    general_settings: settings::General,
    settings_dialog: Option<settings::Dialog>,
    system_theme: Option<egui::Theme>,
    panel_layout: panels::Layout,
    color_panel: colors::ColorPanel,
    layer_thumbnails: thumbnails::LayerThumbnails,
    panel_epoch: u64,
    documents: Vec<workspace::DocumentSlot>,
    open_queue: std::collections::VecDeque<PathBuf>,
    open_errors: Vec<String>,
    active_document_id: u64,
    next_document_id: u64,
    document_order: Vec<u64>,
    initial_placeholder: bool,
    context: egui::Context,
    file_job: Option<FileJob>,
    history_storage_paused: bool,
    after_save: Option<Action>,
    document: Document,
    texture: Option<TextureHandle>,
    canvas_renderer: crate::canvas_renderer::CanvasRenderer,
    texture_dirty: bool,
    tool: Tool,
    color: Rgba,
    secondary_color: Rgba,
    gradient_shape: paint_rust::gradient::GradientShape,
    gradient_mode: paint_rust::gradient::GradientMode,
    brush_size: f32,
    brush_options: paint_rust::brush::Options,
    clone_anchor: Option<(u64, (i32, i32))>,
    clone_offset: Option<(i32, i32)>,
    clone_aligned: bool,
    recolor_once: bool,
    recolor_sample: Option<[u8; 4]>,
    clone_layer: Option<u64>,
    clone_merged: bool,
    shape_filled: bool,
    shape_outline: bool,
    shape_antialias: bool,
    shape_kind: Shape,
    move_selection_only: bool,
    ellipse_selection: bool,
    selection_mode: CombineMode,
    color_tolerance: u8,
    color_alpha: paint_rust::selection::AlphaMatch,
    color_contiguous: bool,
    color_sample_merged: bool,
    polygon_points: Vec<(f64, f64)>,
    lasso: lasso::Lasso,
    polygon_size: (u32, u32),
    selection_texture: Option<(std::sync::Arc<Vec<u8>>, TextureHandle)>,
    shape_origin: Option<(f32, f32)>,
    selection_before: Option<Option<Selection>>,
    zoom: f32,
    canvas_offset: Vec2,
    canvas_viewport: Rect,
    panning: bool,
    pan_button: egui::PointerButton,
    zoom_origin: Option<Pos2>,
    pixel_grid: bool,
    fit_canvas: bool,
    last_point: Option<(f32, f32)>,
    path: Option<PathBuf>,
    status: String,
    pending: Option<Action>,
    new_dialog: bool,
    layer_properties: Option<(String, u8, BlendMode)>,
    resize_dialog: Option<ResizeDialog>,
    export_dialog: Option<export::ExportDialog>,
    export_options: files::RasterOptions,
    text_dialog: Option<TextDialog>,
    system_fonts: Vec<paint_rust::fonts::InstalledFont>,
    font_search: String,
    text_font: Option<(ab_glyph::FontArc, String, u32)>,
    effect_dialog: Option<EffectDialog>,
    selection_transform: Option<selection_transform::Dialog>,
    paste_confirmation: Option<paste::Image>,
    last_effect: Option<Effect>,
    levels_channel: usize,
    effect_parameters: std::collections::HashMap<std::mem::Discriminant<Effect>, Effect>,
    gesture_preview: Option<EffectDialog>,
    keep_shape_editable: bool,
    shape_edit: Option<paint_rust::shapes::Parameters>,
    curve_controls: Option<[(f32, f32); 2]>,
    transform_drag: Option<transform_handles::TransformDrag>,
    new_width: u32,
    new_height: u32,
    new_dpi: paint_rust::resolution::Dpi,
    new_units: paint_rust::resolution::Unit,
    new_link_resolution: bool,
    transparent: bool,
    allow_close: bool,
    suppress_stroke: bool,
    stroke_button: egui::PointerButton,
    appearance: Option<std::sync::mpsc::Receiver<egui::Theme>>,
    clipboard: crate::clipboard::Shared,
    recovery: Option<paint_rust::recovery::Recovery>,
    recoverable: Vec<paint_rust::recovery::Candidate>,
    recovery_prompt: bool,
    recovery_batch: Option<recovery_batch::Batch>,
}

impl Editor {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut editor = Self::blank(&cc.egui_ctx);
        match panels::Layout::load() {
            Ok(layout) => editor.panel_layout = layout,
            Err(error) => editor.status = format!("Using default workspace layout: {error}"),
        }
        if let Err(error) = editor.color_panel.load() {
            editor.status = format!("Using default palette: {error}");
        }
        match preferences::Preferences::load() {
            Ok(settings) => {
                editor.brush_size = settings.brush_size;
                editor.brush_options = settings.brush;
                editor.pixel_grid = settings.pixel_grid;
                editor.preferences = settings;
            }
            Err(error) => editor.status = format!("Using default preferences: {error}"),
        }
        match settings::General::load()
            .and_then(|settings| editor.apply_general_settings(settings, &cc.egui_ctx))
        {
            Ok(()) => {}
            Err(error) => {
                editor.errors.record("load general preferences", &error);
                editor.status = format!("Using default general preferences: {error}");
            }
        }
        if std::env::args_os().len() == 1 {
            match editor.create_new_image() {
                Ok(document) => editor.document = document,
                Err(error) => editor.errors.record("create startup image", &error),
            }
        }
        match effect_settings::Settings::load() {
            Ok(settings) => {
                editor.last_effect = settings.last;
                editor.effect_parameters = settings
                    .parameters
                    .into_iter()
                    .map(|effect| (std::mem::discriminant(&effect), effect))
                    .collect();
                if let Some(last) = editor.last_effect {
                    editor
                        .effect_parameters
                        .insert(std::mem::discriminant(&last), last);
                }
            }
            Err(error) => editor.status = format!("Using default effect settings: {error}"),
        }
        editor.apply_theme(&cc.egui_ctx);
        editor.appearance = Some(crate::appearance::connect(&cc.egui_ctx));
        editor
            .open_queue
            .extend(std::env::args_os().skip(1).map(PathBuf::from));
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
            help_open: false,
            errors: Default::default(),
            about: Default::default(),
            panel_layout: Default::default(),
            color_panel: Default::default(),
            layer_thumbnails: Default::default(),
            panel_epoch: 0,
            documents: Vec::new(),
            open_queue: Default::default(),
            open_errors: Vec::new(),
            active_document_id: 0,
            next_document_id: 1,
            document_order: vec![0],
            initial_placeholder: true,
            context: ctx.clone(),
            file_job: None,
            history_storage_paused: false,
            after_save: None,
            document: Document::new(960, 640, [255; 4]).unwrap(),
            texture: None,
            canvas_renderer: crate::canvas_renderer::CanvasRenderer::new(ctx.clone()),
            texture_dirty: true,
            tool: Tool::Brush,
            color: Rgba::from_rgb(42, 113, 224),
            secondary_color: Rgba::WHITE,
            gradient_shape: paint_rust::gradient::GradientShape::Linear,
            gradient_mode: paint_rust::gradient::GradientMode::Blend,
            brush_size: 12.0,
            brush_options: Default::default(),
            clone_anchor: None,
            clone_offset: None,
            clone_aligned: true,
            recolor_once: false,
            recolor_sample: None,
            clone_layer: None,
            clone_merged: false,
            shape_filled: false,
            shape_outline: false,
            shape_antialias: false,
            shape_kind: Shape::RoundedRectangle,
            move_selection_only: false,
            ellipse_selection: false,
            selection_mode: CombineMode::Replace,
            color_tolerance: 0,
            color_alpha: Default::default(),
            color_contiguous: true,
            color_sample_merged: false,
            polygon_points: Vec::new(),
            lasso: Default::default(),
            polygon_size: (0, 0),
            selection_texture: None,
            shape_origin: None,
            selection_before: None,
            zoom: 1.0,
            canvas_offset: Vec2::ZERO,
            canvas_viewport: Rect::NOTHING,
            panning: false,
            pan_button: egui::PointerButton::Middle,
            zoom_origin: None,
            pixel_grid: false,
            fit_canvas: false,
            last_point: None,
            path: None,
            status: "Ready — draw on the canvas to begin".into(),
            pending: None,
            new_dialog: false,
            layer_properties: None,
            resize_dialog: None,
            export_dialog: None,
            export_options: files::RasterOptions::new(files::RasterFormat::Png),
            text_dialog: None,
            system_fonts: Vec::new(),
            font_search: String::new(),
            text_font: None,
            effect_dialog: None,
            selection_transform: None,
            paste_confirmation: None,
            last_effect: None,
            levels_channel: 0,
            effect_parameters: Default::default(),
            gesture_preview: None,
            keep_shape_editable: false,
            shape_edit: None,
            curve_controls: None,
            transform_drag: None,
            new_width: 960,
            new_height: 640,
            new_dpi: Default::default(),
            new_units: Default::default(),
            new_link_resolution: true,
            transparent: false,
            allow_close: false,
            suppress_stroke: false,
            stroke_button: egui::PointerButton::Primary,
            appearance: None,
            preferences: Default::default(),
            general_settings: Default::default(),
            settings_dialog: None,
            system_theme: None,
            clipboard: Default::default(),
            recovery: None,
            recoverable: Vec::new(),
            recovery_prompt: false,
            recovery_batch: None,
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
        self.start_cancellable_file_job("open image", move |cancel| {
            Ok(Outcome::Opened(
                Box::new(
                    files::open_cancellable(&path, cancel)
                        .map_err(|e| format!("{}: {e}", path.display()))?,
                ),
                path,
            ))
        });
    }

    fn save(&mut self, save_as: bool) -> bool {
        self.finish_stroke();
        let existing = self.path.as_ref().filter(|path| files::writable_path(path));
        let path = if !save_as && existing.is_some() {
            existing.cloned()
        } else {
            rfd::FileDialog::new()
                .add_filter("Paint Rust project", &["prust"])
                .add_filter("PDN layered image", &["pdn"])
                .add_filter("PNG image", &["png"])
                .add_filter("JPEG image", &["jpg", "jpeg"])
                .add_filter("JPEG XL image", &["jxl"])
                .add_filter("AVIF image", &["avif"])
                .add_filter("HEIC image", &["heic", "heif"])
                .add_filter("BMP image", &["bmp"])
                .add_filter("WebP image", &["webp"])
                .add_filter("TIFF image", &["tif", "tiff"])
                .add_filter("TGA image", &["tga"])
                .add_filter("GIF image", &["gif"])
                .add_filter("DDS texture", &["dds"])
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
        self.save_to_path(path)
    }

    fn export_image(&mut self, format: files::RasterFormat) {
        self.finish_stroke();
        let Some(path) = rfd::FileDialog::new()
            .add_filter(format.label(), format.extensions())
            .set_file_name(format!("Untitled.{}", format.extension()))
            .save_file()
        else {
            return;
        };
        let path = if path.extension().is_none() {
            path.with_extension(format.extension())
        } else {
            path
        };
        if !path.extension().is_some_and(|e| {
            format
                .extensions()
                .iter()
                .any(|ext| e.eq_ignore_ascii_case(ext))
        }) {
            self.status = format!(
                "Could not export image: choose a filename ending in .{}",
                format.extension()
            );
            return;
        }
        self.begin_export(path, format);
    }

    fn import_layer(&mut self) {
        self.finish_stroke();
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import image as layer")
            .add_filter(
                "Images and projects",
                &[
                    "png", "jpg", "jpeg", "bmp", "webp", "tif", "tiff", "tga", "gif", "dds", "jxl",
                    "avif", "prust", "pdn",
                ],
            )
            .pick_file()
        else {
            return;
        };
        self.import_layer_from_path(&path);
    }

    fn import_layer_from_path(&mut self, path: &std::path::Path) {
        let path = path.to_path_buf();
        self.start_cancellable_file_job("import image", move |cancel| {
            let image = files::open_cancellable(&path, cancel)?;
            Ok(Outcome::Imported(
                image
                    .composite_cancellable(cancel)
                    .ok_or(files::CANCELLED)?,
                image.width,
                image.height,
                path,
            ))
        });
    }

    fn begin_text(&mut self, ctx: &egui::Context, origin: (f32, f32), color: Rgba) {
        let data = ctx.fonts(|fonts| {
            let definitions = fonts.definitions();
            definitions
                .families
                .get(&egui::FontFamily::Proportional)
                .and_then(|names| names.first())
                .and_then(|name| definitions.font_data.get(name))
                .cloned()
        });
        let default_index = data.as_ref().map_or(0, |data| data.index);
        let font = data
            .and_then(|data| {
                ab_glyph::FontVec::try_from_vec_and_index(data.font.to_vec(), data.index).ok()
            })
            .map(ab_glyph::FontArc::new);
        let (font, font_name, font_index) = if let Some((font, name, index)) = &self.text_font {
            (Some(font.clone()), name.clone(), *index)
        } else {
            (font, "System UI font".into(), default_index)
        };
        if let Some(font) = font {
            self.document.begin("Text");
            let preview = crate::preview::EffectPreview::new_text(
                ctx.clone(),
                self.document.shared_pixels(),
                self.document.width,
                self.document.height,
                self.document.selection(),
                crate::preview::TextInput {
                    layout: paint_rust::text::TextLayout {
                        font_index,
                        ..Default::default()
                    },
                    font: font.clone(),
                    content: String::new(),
                    origin,
                    size: 32.0,
                    color: color.to_srgba_unmultiplied(),
                },
            );
            self.text_dialog = Some(TextDialog {
                font_name,
                layout: paint_rust::text::TextLayout {
                    font_index,
                    ..Default::default()
                },
                preview,
                content: String::new(),
                font,
                origin,
                size: 32.0,
                color,
                focus: true,
            });
        } else {
            self.status = "Could not load a font for the text tool".into();
        }
    }

    fn copy(&mut self, cut: bool, merged: bool) {
        self.finish_stroke();
        let mut document = self.document.snapshot();
        document.set_selection(self.document.selection());
        let clipboard = self.clipboard.clone();
        self.start_cancellable_file_job(
            if cut { "cut image" } else { "copy image" },
            move |cancel| {
                let (width, height, pixels) = document
                    .copy_pixels_cancellable(merged, cancel)
                    .ok_or(files::CANCELLED)?;
                if width == 0 || height == 0 {
                    return Err("Selection is empty".into());
                }
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return Err(files::CANCELLED.into());
                }
                crate::clipboard::with_backend(&clipboard, |backend| {
                    backend.write(width, height, &pixels)
                })?;
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return Err(files::CANCELLED.into());
                }
                if cut {
                    document.fill_selection_cancellable([255, 255, 255, 0], cancel)?;
                    document.set_selection(None);
                }
                Ok(Outcome::Copied(cut.then(|| Box::new(document))))
            },
        );
    }

    fn paste_new_image(&mut self) {
        self.finish_stroke();
        let clipboard = self.clipboard.clone();
        self.start_cancellable_file_job("paste new image", move |cancel| {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(files::CANCELLED.into());
            }
            let (width, height, pixels) =
                crate::clipboard::with_backend(&clipboard, |backend| backend.read())?;
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(files::CANCELLED.into());
            }
            let mut document = Document::from_pixels(width, height, pixels)?;
            document.mark_unsaved();
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(files::CANCELLED.into());
            }
            Ok(Outcome::PastedImage(Box::new(document)))
        });
    }

    fn paste(&mut self) {
        self.paste_with_mode(false);
    }
    fn paste_new_layer(&mut self) {
        self.paste_with_mode(true);
    }
    fn paste_with_mode(&mut self, new_layer: bool) {
        self.finish_stroke();
        if new_layer && !self.document.can_add_layer() {
            self.status = "Cannot add another layer to this document".into();
            return;
        }
        let clipboard = self.clipboard.clone();
        self.start_cancellable_file_job("read clipboard image", move |cancel| {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(files::CANCELLED.into());
            }
            let (width, height, pixels) =
                crate::clipboard::with_backend(&clipboard, |backend| backend.read())?;
            Document::validate_size(width, height)?;
            if pixels.len() != width as usize * height as usize {
                return Err("Invalid clipboard pixel buffer".into());
            }
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(files::CANCELLED.into());
            }
            Ok(Outcome::ClipboardImage(paste::Image {
                width,
                height,
                pixels,
                new_layer,
            }))
        });
    }

    fn request(&mut self, action: Action, ctx: &egui::Context) {
        self.polygon_points.clear();
        self.lasso.cancel();
        if self.file_job.is_some() {
            return;
        }
        self.finish_stroke();
        if self.document.dirty()
            && matches!(
                action,
                Action::Close | Action::CloseDocument | Action::CloseAll
            )
        {
            self.pending = Some(action);
        } else {
            self.perform(action, ctx);
        }
    }

    fn perform(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::SaveAll => self.save_all_next(),
            Action::New => self.new_dialog = true,
            Action::Recovery => self.recovery_prompt = true,
            Action::Open => {
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter(
                        "Projects and images",
                        &[
                            "prust", "pdn", "png", "jpg", "jpeg", "bmp", "webp", "tif", "tiff",
                            "tga", "gif", "dds", "jxl", "avif", "heic", "heif",
                        ],
                    )
                    .pick_files()
                {
                    self.open_queue.extend(paths);
                }
            }
            Action::CloseDocument => self.close_current_document(),
            Action::CloseAll => {
                let more = !self.documents.is_empty();
                self.close_current_document();
                if more {
                    self.request(Action::CloseAll, ctx);
                }
            }
            Action::Close => {
                if !self.documents.is_empty() {
                    self.close_current_document();
                    self.request(Action::Close, ctx);
                    return;
                }
                self.clear_recovery();
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn request_gesture_preview(&mut self, effect: Effect) {
        if let Some(gesture) = &mut self.gesture_preview {
            if gesture.effect != effect {
                gesture.effect = effect;
                gesture.preview.request(effect);
            }
        } else {
            let preview = crate::preview::EffectPreview::new(
                self.context.clone(),
                self.document.shared_pixels(),
                self.document.width,
                self.document.height,
                self.document.selection(),
                effect,
            );
            self.gesture_preview = Some(EffectDialog {
                effect,
                preview,
                histogram: None,
                output_histogram: None,
            });
        }
    }

    fn finish_stroke(&mut self) {
        if self.shape_edit.is_some() {
            return;
        }
        if self.text_dialog.is_some()
            || self.effect_dialog.is_some()
            || self.selection_transform.is_some()
            || self.paste_confirmation.is_some()
        {
            return;
        }
        if let Some(gesture) = self.gesture_preview.take()
            && !gesture.preview.ready
        {
            self.document.cancel();
            self.texture_dirty = true;
            let mut document = self.document.snapshot();
            document.set_selection(self.document.selection());
            self.recolor_sample = None;
            self.last_point = None;
            self.shape_origin = None;
            let label = gesture.effect.name();
            self.start_cancellable_file_job("finish drawing", move |cancel| {
                let pixels = gesture.preview.wait(cancel)?;
                document.begin(label);
                let moved = if let Effect::Move(parameters) = gesture.effect {
                    document.publish_move_preview(pixels, parameters.dx, parameters.dy)?;
                    true
                } else {
                    document.publish_preview(pixels)?;
                    false
                };
                document.commit();
                Ok(Outcome::Edited(Box::new(document), label.into(), moved))
            });
            return;
        }
        if self.document.editing() {
            self.document.commit();
            self.texture_dirty = true;
        }
        if self.tool == Tool::Select
            && let Some(previous) = self.selection_before.take()
        {
            let candidate = self.document.selection();
            self.document.set_selection(previous.clone());
            if self.selection_mode == CombineMode::Replace {
                self.document.set_selection_with_history(
                    candidate,
                    if self.ellipse_selection {
                        "Ellipse selection"
                    } else {
                        "Rectangle selection"
                    },
                );
            } else if let Some(candidate) = candidate {
                let (width, height, mode) = (
                    self.document.width,
                    self.document.height,
                    self.selection_mode,
                );
                self.start_cancellable_file_job("combine selections", move |cancel| {
                    paint_rust::selection::combine_cancellable(
                        width,
                        height,
                        previous.as_ref(),
                        &candidate,
                        mode,
                        cancel,
                    )
                    .map(Outcome::Selection)
                });
            }
        }
        self.recolor_sample = None;
        self.last_point = None;
        self.shape_origin = None;
        self.selection_before = None;
    }

    fn undo(&mut self) {
        self.navigate_history(1, false);
    }
    fn redo(&mut self) {
        self.navigate_history(1, true);
    }

    fn navigate_history(&mut self, steps: usize, redo: bool) {
        if self.file_job.is_some()
            || self.text_dialog.is_some()
            || self.effect_dialog.is_some()
            || self.selection_transform.is_some()
            || self.paste_confirmation.is_some()
        {
            return;
        }
        self.finish_stroke();
        let available = if redo {
            self.document.redo_history().count()
        } else {
            self.document.history().count()
        };
        if self.document.history_navigation_needs_io(steps, redo) {
            self.navigate_history_async(steps.min(available), redo);
            return;
        }
        let mut count = 0;
        for _ in 0..steps.min(available) {
            let changed = if redo {
                self.document.redo()
            } else {
                self.document.undo()
            };
            if !changed {
                break;
            }
            count += 1;
        }
        self.texture_dirty |= count > 0;
        self.status = self
            .document
            .history_error()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "{} {count} operation(s)",
                    if redo { "Redone" } else { "Undone" }
                )
            });
    }

    fn begin_effect(&mut self, ctx: &egui::Context, effect: Effect) {
        let effect = if effect.repeatable() {
            self.effect_parameters
                .get(&std::mem::discriminant(&effect))
                .copied()
                .unwrap_or(effect)
        } else {
            effect
        };
        self.transform_drag = None;
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
        let histogram = matches!(
            effect,
            Effect::Levels(_) | Effect::AutoLevels(_) | Effect::Curves(_)
        )
        .then(|| {
            histogram::Histogram::new(
                ctx.clone(),
                self.document.shared_pixels(),
                self.document.width,
                self.document.selection(),
            )
        });
        self.effect_dialog = Some(EffectDialog {
            effect,
            preview,
            histogram,
            output_histogram: None,
        });
    }

    fn apply_effect(&mut self, effect: Effect) {
        if effect.repeatable() {
            self.last_effect = Some(effect);
            self.effect_parameters
                .insert(std::mem::discriminant(&effect), effect);
        }
        let changed = self.document.commit();
        if changed && matches!(effect, Effect::FreeTransform(_)) {
            self.document.set_selection(None);
        }
        self.texture_dirty = true;
    }

    fn complete_polygon(&mut self) {
        if self.polygon_points.len() < 3 {
            self.status = "Add at least three polygon points".into();
            return;
        }
        let points = std::mem::take(&mut self.polygon_points);
        let (width, height) = (self.document.width, self.document.height);
        let previous = self.document.selection();
        let mode = self.selection_mode;
        self.start_cancellable_file_job("select polygon", move |cancel| {
            let candidate =
                paint_rust::selection::polygon_cancellable(width, height, &points, cancel)?;
            paint_rust::selection::combine_cancellable(
                width,
                height,
                previous.as_ref(),
                &candidate,
                mode,
                cancel,
            )
            .map(Outcome::Selection)
        });
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.shape_edit.is_some() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
                self.finish_shape_edit(false);
            } else if !ctx.wants_keyboard_input()
                && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter))
            {
                self.finish_shape_edit(true);
            }
            return;
        }
        if self.new_dialog && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.new_dialog = false;
            return;
        }
        if self.about.open && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.about.open = false;
            return;
        }
        if self.errors.open && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
        {
            self.errors.open = false;
            return;
        }
        if self.help_open && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.help_open = false;
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, Key::X)) {
            self.begin_settings();
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::F1)) {
            self.errors.open = true;
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::F1)) {
            self.help_open = true;
            return;
        }
        if ctx.wants_keyboard_input() {
            return;
        }
        if self.tool == Tool::Polygon && !self.polygon_points.is_empty() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
                self.polygon_points.clear();
                self.status = "Polygon selection cancelled".into();
            } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter)) {
                self.complete_polygon();
            } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Backspace)) {
                self.polygon_points.pop();
            }
            return;
        }

        if (self.tool == Tool::Select || self.gesture_preview.is_some())
            && self.last_point.is_some()
            && !ctx.input(|i| i.key_pressed(Key::Escape))
        {
            return;
        }
        if self.structure_shortcuts(ctx) {
            return;
        }
        if self.tool == Tool::Move && !self.document.editing() && self.last_point.is_none() {
            let modifiers = ctx.input(|i| i.modifiers);
            if !modifiers.alt {
                let delta = ctx.input_mut(|i| {
                    let mut delta = (0, 0);
                    for (key, dx, dy) in [
                        (Key::ArrowLeft, -1, 0),
                        (Key::ArrowRight, 1, 0),
                        (Key::ArrowUp, 0, -1),
                        (Key::ArrowDown, 0, 1),
                    ] {
                        if i.consume_key(modifiers, key) {
                            delta.0 += dx;
                            delta.1 += dy;
                        }
                    }
                    delta
                });
                if delta != (0, 0) {
                    let step = if modifiers.shift { 10 } else { 1 };
                    self.document.begin(if self.move_selection_only {
                        "Nudge selection"
                    } else if modifiers.command {
                        "Copy pixels"
                    } else {
                        "Nudge pixels"
                    });
                    if self.move_selection_only {
                        self.document.preview_move(
                            delta.0 * step,
                            delta.1 * step,
                            modifiers.command,
                            true,
                        );
                        self.document.commit();
                        self.status = "Moved selection bounds".into();
                    } else {
                        self.request_gesture_preview(Effect::Move(
                            paint_rust::movement::Parameters {
                                dx: delta.0 * step,
                                dy: delta.1 * step,
                                copy: modifiers.command,
                            },
                        ));
                        self.finish_stroke();
                    }
                    return;
                }
            }
        }
        ctx.input(|i| {
            for (index, key) in [Key::F5, Key::F6, Key::F7, Key::F8].iter().enumerate() {
                if i.key_pressed(*key) {
                    if i.modifiers.ctrl && i.modifiers.shift {
                        self.panel_layout.rects[index] = None;
                        self.panel_layout.visible[index] = true;
                        self.panel_epoch += 1;
                    } else {
                        self.panel_layout.visible[index] = !self.panel_layout.visible[index];
                    }
                }
            }
        });
        let command = ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::COMMAND, Key::Plus) || i.consume_key(egui::Modifiers::COMMAND, Key::Equals) {Some(34)
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::Minus) {Some(35)
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::Num0) {Some(36)
            } else if i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::ALT | egui::Modifiers::SHIFT,Key::C) {Some(28)
            } else if i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,Key::C) {Some(30)
            } else if i.events.iter().any(|event| matches!(event,egui::Event::Copy)) {Some(if i.modifiers.alt && i.modifiers.shift {28}else if i.modifiers.shift {30}else{11})
            } else if i.events.iter().any(|event| matches!(event,egui::Event::Cut)) { Some(12)
            // The window backend consumes Ctrl+V presses when no text is available.
            // Its release event remains available for image clipboard handling.
            } else if i.events.iter().any(|event| matches!(event,egui::Event::Key{key:Key::V,pressed:false,modifiers,..} if modifiers.command && modifiers.alt && modifiers.shift)) {Some(29)
            } else if i.events.iter().any(|event| matches!(event,egui::Event::Key{key:Key::V,pressed:false,modifiers,..} if modifiers.command && modifiers.alt)) { Some(27)
            } else if i.events.iter().any(|event| matches!(event,egui::Event::Key{key:Key::V,pressed:false,modifiers,..} if modifiers.command && modifiers.shift)) {Some(32)
            } else if i.events.iter().any(|event| matches!(event,egui::Event::Key{key:Key::V,pressed:false,modifiers,..} if modifiers.command)) { Some(13)
            } else if i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::Tab) {
                Some(16)
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::Tab) {
                Some(15)
            } else if i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::I) {
                Some(21)
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::I) {
                Some(20)
            } else if i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::ALT, Key::F) {
                Some(19)
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::F) {
                Some(18)
            } else if i.consume_key(egui::Modifiers::SHIFT,Key::Backspace) {Some(33)
            } else if i.consume_key(egui::Modifiers::NONE,Key::Backspace) {Some(31)
            } else if i.consume_key(egui::Modifiers::NONE, Key::F4) {
                Some(26)
            } else if i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::ALT, Key::M) {
                Some(25)
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::M) {
                Some(17)
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::W) {
                Some(14)
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::N) {
                Some(0)
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::O) {
                Some(1)
            } else if i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::B) {
                Some(24)
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::B) {
                Some(23)
            } else if i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::ALT, Key::S) {
                Some(22)
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
            } else if i.consume_key(egui::Modifiers::COMMAND, Key::D) || i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::A) {
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
                if i.key_pressed(Key::U) { self.tool = Tool::Shapes; }
                if i.key_pressed(Key::C) { self.tool = Tool::Clone; }
                if i.key_pressed(Key::J) { self.tool = Tool::Recolor; }
                if i.key_pressed(Key::Q) { self.tool = Tool::Lasso; }
                if i.key_pressed(Key::W) { self.tool = Tool::ColorSelect; }
                if i.key_pressed(Key::P) { self.tool = Tool::Polygon; }
                if i.key_pressed(Key::Z) { self.tool = Tool::Zoom; }
                if i.key_pressed(Key::H) { self.tool = Tool::Pan; }
                if i.key_pressed(Key::N) { self.tool = Tool::Pencil; }
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
            Some(34) => self.step_view_zoom(true),
            Some(35) => self.step_view_zoom(false),
            Some(36) => self.set_view_zoom(1.0),
            Some(32) => self.paste_new_layer(),
            Some(31) => {
                self.start_document_edit(DocumentEdit::Fill(self.color.to_srgba_unmultiplied()))
            }
            Some(33) => self.start_document_edit(DocumentEdit::Fill(
                self.secondary_color.to_srgba_unmultiplied(),
            )),
            Some(28) => self.copy_selection_geometry(),
            Some(29) => self.paste_selection_geometry(CombineMode::Replace),
            Some(30) => self.copy(false, true),
            Some(27) => self.paste_new_image(),
            Some(26) => {
                let layer = &self.document.layers()[self.document.active_layer()];
                self.layer_properties = Some((layer.name.clone(), layer.opacity, layer.blend_mode));
            }
            Some(25) => self.begin_selection_transform(ctx),
            Some(23) => self.fit_canvas = true,
            Some(24) => self.zoom_to_selection(),
            Some(22) => self.request(Action::SaveAll, ctx),
            Some(20) => self.invert_selection(),
            Some(21) => self.start_document_edit(DocumentEdit::Invert),
            Some(18) => self.repeat_effect(),
            Some(19) => {
                if let Some(effect) = self.last_effect {
                    self.begin_effect(ctx, effect);
                }
            }
            Some(17) => {
                if self.document.can_merge_down() {
                    self.start_document_edit(DocumentEdit::MergeDown);
                }
            }
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
                self.lasso.cancel();
                self.zoom_origin = None;
                self.suppress_stroke = true;
                self.gesture_preview = None;
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
                self.document.set_selection_with_history(
                    Some(Selection {
                        mask: None,
                        ellipse: None,
                        x: 0,
                        y: 0,
                        width: self.document.width,
                        height: self.document.height,
                    }),
                    "Select all",
                );
            }
            Some(8) => {
                self.finish_stroke();
                self.document.set_selection_with_history(None, "Deselect");
            }
            Some(9) => {
                self.finish_stroke();
                self.start_document_edit(DocumentEdit::Delete);
            }
            Some(10) => {
                self.finish_stroke();
                self.transform_in_background("crop image", Document::crop_to_selection_cancellable);
            }
            Some(11) => self.copy(false, false),
            Some(12) => self.copy(true, false),
            Some(13) => self.paste(),
            Some(14) => self.request(Action::CloseDocument, ctx),
            Some(15) => self.cycle_document(1),
            Some(16) => self.cycle_document(-1),
            _ => {}
        }
    }

    fn canvas_content(&mut self, ui: &mut egui::Ui) {
        if let Some(update) = self.canvas_renderer.poll() {
            match update {
                crate::canvas_renderer::Update::Full(image) => {
                    if let Some(texture) = &mut self.texture {
                        texture.set(image, TextureOptions::NEAREST);
                    } else {
                        self.texture = Some(ui.ctx().load_texture(
                            "canvas",
                            image,
                            TextureOptions::NEAREST,
                        ));
                    }
                }
                crate::canvas_renderer::Update::Partial { position, image } => {
                    if let Some(texture) = &mut self.texture {
                        texture.set_partial(position, image, TextureOptions::NEAREST);
                    } else {
                        self.canvas_renderer.invalidate();
                        self.texture_dirty = true;
                    }
                }
                crate::canvas_renderer::Update::Unchanged => {}
            }
        }
        // Let an in-flight stroke preview finish; cancelling it every pointer
        // frame can starve large composites and repeatedly clone the pixel buffer.
        if self.texture_dirty && !(self.document.editing() && self.canvas_renderer.pending) {
            self.canvas_renderer.request(&self.document);
            self.texture_dirty = false;
        }
        if self.document.selection().is_none() {
            self.selection_texture = None;
        }
        if self.tool != Tool::Polygon
            || self.polygon_size != (self.document.width, self.document.height)
        {
            self.polygon_points.clear();
            self.polygon_size = (self.document.width, self.document.height);
        }
        let viewport = ui.available_rect_before_wrap();
        if self.fit_canvas {
            let available = viewport.size();
            self.zoom = (available.x / self.document.width as f32)
                .min(available.y / self.document.height as f32)
                .clamp(0.01, 64.0);
            self.canvas_offset = Vec2::ZERO;
            self.fit_canvas = false;
        }
        let navigation_enabled =
            ui.is_enabled() && !self.document.editing() && self.last_point.is_none();
        let temporary_pan =
            ui.input(|i| i.key_down(Key::Space)) && !ui.ctx().wants_keyboard_input();
        let pointer = ui.input(|i| i.pointer.hover_pos());
        if navigation_enabled && let Some(pointer) = pointer.filter(|p| viewport.contains(*p)) {
            let factor = ui.input(|i| i.zoom_delta());
            if factor != 1.0 {
                let zoom = (self.zoom * factor).clamp(0.01, 64.0);
                let anchor = pointer - viewport.min;
                self.canvas_offset =
                    ((self.canvas_offset + anchor) * (zoom / self.zoom) - anchor).max(Vec2::ZERO);
                self.zoom = zoom;
            }
            if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Middle)) {
                self.panning = true;
                self.pan_button = egui::PointerButton::Middle;
            } else if (self.tool == Tool::Pan || temporary_pan)
                && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
            {
                self.panning = true;
                self.pan_button = egui::PointerButton::Primary;
                self.stroke_button = egui::PointerButton::Primary;
                self.suppress_stroke = true;
            }
        }
        if !navigation_enabled || !ui.input(|i| i.pointer.button_down(self.pan_button)) {
            self.panning = false;
        }
        if self.panning {
            if !ui.input(|i| i.pointer.button_pressed(self.pan_button)) {
                self.canvas_offset =
                    (self.canvas_offset - ui.input(|i| i.pointer.delta())).max(Vec2::ZERO);
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        if self.tool != Tool::Zoom || !navigation_enabled || !ui.input(|i| i.focused) {
            self.zoom_origin = None;
        }
        let mut requested_view = None;
        let scroll = egui::ScrollArea::both()
            .id_salt("canvas-scroll")
            .scroll_offset(self.canvas_offset)
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let size =
                    Vec2::new(self.document.width as f32, self.document.height as f32) * self.zoom;
                let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Image,
                        response.enabled(),
                        format!(
                            "Image canvas, {} by {} pixels",
                            self.document.width, self.document.height
                        ),
                    )
                });
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
                if let Some(texture) = &self.texture
                    && texture.size()
                        == [self.document.width as usize, self.document.height as usize]
                {
                    painter.image(
                        texture.id(),
                        rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                if let Some(selection) = self.document.selection().filter(|_| {
                    !self
                        .effect_dialog
                        .as_ref()
                        .is_some_and(|dialog| matches!(dialog.effect, Effect::FreeTransform(_)))
                }) {
                    if let Some(mask) = &selection.mask {
                        if self
                            .selection_texture
                            .as_ref()
                            .is_none_or(|(pixels, _)| !std::sync::Arc::ptr_eq(pixels, &mask.pixels))
                        {
                            let pixels = mask
                                .pixels
                                .iter()
                                .map(|value| {
                                    if *value == 0 {
                                        Color32::TRANSPARENT
                                    } else {
                                        Color32::from_rgba_unmultiplied(40, 120, 255, 60)
                                    }
                                })
                                .collect();
                            let texture = ui.ctx().load_texture(
                                "selection-mask",
                                egui::ColorImage::new(
                                    [mask.width as usize, mask.height as usize],
                                    pixels,
                                ),
                                TextureOptions::NEAREST,
                            );
                            self.selection_texture = Some((mask.pixels.clone(), texture));
                        }
                        if let Some((_, texture)) = &self.selection_texture {
                            let area = Rect::from_min_size(
                                rect.min + Vec2::new(mask.x as f32, mask.y as f32) * self.zoom,
                                Vec2::new(mask.width as f32, mask.height as f32) * self.zoom,
                            );
                            painter.image(
                                texture.id(),
                                area,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                    } else {
                        self.selection_texture = None;
                    }
                    let outline = Rect::from_min_size(
                        rect.min + Vec2::new(selection.x as f32, selection.y as f32) * self.zoom,
                        Vec2::new(selection.width as f32, selection.height as f32) * self.zoom,
                    );
                    if let Some(ellipse) = selection.ellipse {
                        let center = rect.min
                            + Vec2::new(
                                ellipse.x as f32 + ellipse.width as f32 / 2.0,
                                ellipse.y as f32 + ellipse.height as f32 / 2.0,
                            ) * self.zoom;
                        let radius = Vec2::new(ellipse.width as f32, ellipse.height as f32)
                            * (self.zoom / 2.0);
                        for (width, color) in [(3.0_f32, Color32::BLACK), (1.0_f32, Color32::WHITE)]
                        {
                            painter.add(egui::epaint::EllipseShape::stroke(
                                center,
                                radius,
                                egui::Stroke::new(width, color),
                            ));
                        }
                    } else {
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
                }
                if self.pixel_grid && self.zoom >= 8.0 {
                    let first = ((clip.min - rect.min) / self.zoom).floor().max(Vec2::ZERO);
                    let last = ((clip.max - rect.min) / self.zoom).ceil();
                    let stroke = egui::Stroke::new(1.0_f32, Color32::from_black_alpha(90));
                    for x in first.x as u32..=(last.x as u32).min(self.document.width) {
                        let x = rect.min.x + x as f32 * self.zoom;
                        painter.line_segment(
                            [Pos2::new(x, clip.min.y), Pos2::new(x, clip.max.y)],
                            stroke,
                        );
                    }
                    for y in first.y as u32..=(last.y as u32).min(self.document.height) {
                        let y = rect.min.y + y as f32 * self.zoom;
                        painter.line_segment(
                            [Pos2::new(clip.min.x, y), Pos2::new(clip.max.x, y)],
                            stroke,
                        );
                    }
                }
                if self.tool == Tool::Zoom && navigation_enabled && !self.panning && !temporary_pan
                {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ZoomIn);
                    if response.drag_started_by(egui::PointerButton::Primary) {
                        self.zoom_origin = ui.input(|i| i.pointer.press_origin());
                    }
                    if let Some(origin) = self.zoom_origin
                        && let Some(pointer) = ui.input(|i| i.pointer.latest_pos())
                    {
                        let region = Rect::from_two_pos(origin, pointer).intersect(rect);
                        painter.rect_stroke(
                            region,
                            0.0,
                            egui::Stroke::new(1.0_f32, Color32::from_rgb(53, 132, 228)),
                            egui::StrokeKind::Inside,
                        );
                        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
                            self.zoom_origin = None;
                            if region.width() >= 4.0 && region.height() >= 4.0 {
                                let image_size = region.size() / self.zoom;
                                let zoom = (viewport.width() / image_size.x)
                                    .min(viewport.height() / image_size.y)
                                    .clamp(0.01, 64.0);
                                requested_view =
                                    Some((zoom, (region.min - rect.min) / self.zoom * zoom));
                            }
                        }
                    } else if (response.clicked_by(egui::PointerButton::Primary)
                        || response.clicked_by(egui::PointerButton::Secondary))
                        && let Some(pointer) = response.interact_pointer_pos()
                    {
                        let factor = if response.clicked_by(egui::PointerButton::Secondary) {
                            0.5
                        } else {
                            2.0
                        };
                        let zoom = (self.zoom * factor).clamp(0.01, 64.0);
                        let anchor = pointer - viewport.min;
                        requested_view = Some((
                            zoom,
                            ((self.canvas_offset + anchor) * (zoom / self.zoom) - anchor)
                                .max(Vec2::ZERO),
                        ));
                    }
                }
                self.lasso_canvas(ui, &response, rect, &painter, temporary_pan);
                self.transform_handles(ui, rect, &painter);
                if response.hovered()
                    && !self.panning
                    && matches!(
                        self.tool,
                        Tool::Brush | Tool::Pencil | Tool::Eraser | Tool::Clone | Tool::Recolor
                    )
                {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                    if let Some(pos) = response.hover_pos() {
                        let diameter = if self.tool == Tool::Pencil {
                            1.0
                        } else {
                            self.brush_size
                        };
                        let radius = diameter * self.zoom / 2.0;
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
                if ui.is_enabled() && self.tool == Tool::Polygon && !self.panning && !temporary_pan
                {
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                    }
                    if response.clicked_by(egui::PointerButton::Primary)
                        && let Some(pos) = response.interact_pointer_pos()
                    {
                        if self.polygon_points.len() < 4096 {
                            let point = (pos - rect.min) / self.zoom;
                            self.polygon_points
                                .push((f64::from(point.x), f64::from(point.y)));
                        } else {
                            self.status = "A polygon can contain at most 4096 points".into();
                        }
                        if response.double_clicked_by(egui::PointerButton::Primary)
                            && self.polygon_points.len() >= 3
                        {
                            self.complete_polygon();
                        }
                    }
                    let mut points: Vec<Pos2> = self
                        .polygon_points
                        .iter()
                        .map(|(x, y)| rect.min + Vec2::new(*x as f32, *y as f32) * self.zoom)
                        .collect();
                    if !points.is_empty()
                        && let Some(pointer) = response.hover_pos()
                    {
                        points.push(pointer);
                    }
                    for segment in points.windows(2) {
                        painter.line_segment(
                            [segment[0], segment[1]],
                            egui::Stroke::new(3.0_f32, Color32::BLACK),
                        );
                        painter.line_segment(
                            [segment[0], segment[1]],
                            egui::Stroke::new(1.0_f32, Color32::WHITE),
                        );
                    }
                    for point in points.iter().take(self.polygon_points.len()) {
                        painter.circle_filled(*point, 3.0, Color32::WHITE);
                        painter.circle_stroke(
                            *point,
                            3.0,
                            egui::Stroke::new(1.0_f32, Color32::BLACK),
                        );
                    }
                }
                let drawing_button = if self.last_point.is_some() {
                    self.stroke_button
                } else if matches!(
                    self.tool,
                    Tool::Brush
                        | Tool::Recolor
                        | Tool::Pencil
                        | Tool::Fill
                        | Tool::Picker
                        | Tool::Line
                        | Tool::Shapes
                        | Tool::Rectangle
                        | Tool::Ellipse
                        | Tool::Gradient
                        | Tool::Text
                ) && (ui.input(|i| i.pointer.secondary_down())
                    || response.clicked_by(egui::PointerButton::Secondary))
                {
                    egui::PointerButton::Secondary
                } else {
                    egui::PointerButton::Primary
                };
                self.clone_source(ui, &response, rect, &painter, temporary_pan);
                self.shape_handles(ui, rect);
                if ui.is_enabled()
                    && self.shape_edit.is_none()
                    && (self.tool != Tool::Clone
                        || (self
                            .clone_anchor
                            .is_some_and(|(id, _)| id == self.active_document_id)
                            && !ui.input(|i| i.modifiers.ctrl)))
                    && !matches!(
                        self.tool,
                        Tool::Polygon | Tool::Lasso | Tool::Pan | Tool::Zoom
                    )
                    && !temporary_pan
                    && ui.input(|i| i.focused)
                    && !self.suppress_stroke
                    && !self.panning
                    && ((response.is_pointer_button_down_on()
                        && ui.input(|i| i.pointer.button_down(drawing_button)))
                        || response.clicked_by(drawing_button)
                        || (ui.input(|i| i.pointer.button_released(drawing_button))
                            && self.last_point.is_some()))
                    && let Some(pos) = response
                        .interact_pointer_pos()
                        .or_else(|| ui.input(|i| i.pointer.latest_pos()))
                {
                    let local = (pos - rect.min) / self.zoom;
                    let point = (local.x, local.y);
                    let drawing_color = if drawing_button == egui::PointerButton::Secondary {
                        self.secondary_color
                    } else {
                        self.color
                    };

                    if self.last_point.is_none() {
                        self.stroke_button = drawing_button;
                        let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
                        let origin = (origin - rect.min) / self.zoom;
                        self.shape_origin = Some((origin.x, origin.y));
                        match self.tool {
                            Tool::Clone => {
                                if (!self.clone_aligned || self.clone_offset.is_none())
                                    && let Some((_, anchor)) = self.clone_anchor
                                {
                                    self.clone_offset = Some((
                                        anchor.0 - origin.x.floor() as i32,
                                        anchor.1 - origin.y.floor() as i32,
                                    ));
                                }
                                if let Err(error) = self.document.prepare_clone_source(
                                    self.clone_layer.unwrap_or(0),
                                    self.clone_merged,
                                ) {
                                    self.status = error;
                                    self.shape_origin = None;
                                    return;
                                }
                                self.document.begin("Clone stamp");
                            }
                            Tool::Recolor => {
                                let (x, y) = (point.0.floor() as i64, point.1.floor() as i64);
                                self.recolor_sample = if x >= 0
                                    && y >= 0
                                    && x < i64::from(self.document.width)
                                    && y < i64::from(self.document.height)
                                {
                                    Some(
                                        self.document.pixels()[y as usize
                                            * self.document.width as usize
                                            + x as usize],
                                    )
                                } else {
                                    None
                                };
                                self.document.begin("Recolor");
                            }
                            Tool::Pencil => self.document.begin("Pencil stroke"),
                            Tool::Gradient => self.document.begin("Gradient"),
                            Tool::Move => self.document.begin(if self.move_selection_only {
                                "Move selection"
                            } else {
                                "Move pixels"
                            }),
                            Tool::Text => self.begin_text(ui.ctx(), point, drawing_color),
                            Tool::Select => {
                                self.selection_before = Some(self.document.selection());
                            }
                            Tool::Polygon | Tool::Lasso | Tool::Pan | Tool::Zoom => {}
                            Tool::Picker => {
                                if let Some(pixel) = self
                                    .document
                                    .sample(local.x.floor() as i32, local.y.floor() as i32)
                                {
                                    let sampled = Rgba::from_rgba_unmultiplied(
                                        pixel[0], pixel[1], pixel[2], pixel[3],
                                    );
                                    if drawing_button == egui::PointerButton::Secondary {
                                        self.secondary_color = sampled;
                                    } else {
                                        self.color = sampled;
                                    }
                                }
                            }
                            Tool::ColorSelect => {
                                self.start_color_selection(
                                    local.x.floor() as u32,
                                    local.y.floor() as u32,
                                );
                            }
                            Tool::Fill => {
                                self.start_fill(
                                    local.x.floor() as i32,
                                    local.y.floor() as i32,
                                    drawing_color.to_srgba_unmultiplied(),
                                );
                            }
                            Tool::Line => self.document.begin("Line"),
                            Tool::Shapes => self.document.begin(self.shape_kind.label()),
                            Tool::Rectangle => self.document.begin("Rectangle"),
                            Tool::Ellipse => self.document.begin("Ellipse"),
                            _ => self.document.begin(if self.tool == Tool::Eraser {
                                "Eraser"
                            } else {
                                "Brush stroke"
                            }),
                        }
                    }
                    if self.tool == Tool::Pencil {
                        let color = if drawing_button == egui::PointerButton::Secondary {
                            self.secondary_color
                        } else {
                            self.color
                        };
                        self.document.pencil_with_mode(
                            self.last_point.or(self.shape_origin).unwrap_or(point),
                            point,
                            color.to_srgba_unmultiplied(),
                            self.brush_options
                                .blend
                                .then_some(self.brush_options.blend_mode),
                        );
                        self.texture_dirty = true;
                    }
                    if matches!(self.tool, Tool::Brush | Tool::Eraser) {
                        self.document.brush_segment(
                            self.last_point.or(self.shape_origin).unwrap_or(point),
                            point,
                            self.brush_size,
                            drawing_color.to_srgba_unmultiplied(),
                            self.brush_options,
                            self.tool == Tool::Eraser,
                        );
                        self.texture_dirty = true;
                    }
                    if matches!(self.tool, Tool::Clone | Tool::Recolor) {
                        let paint = if self.tool == Tool::Clone {
                            paint_rust::brush::Paint::Clone {
                                offset: self.clone_offset.unwrap_or((0, 0)),
                            }
                        } else {
                            let from = if drawing_button == egui::PointerButton::Secondary {
                                self.color
                            } else {
                                self.secondary_color
                            };
                            paint_rust::brush::Paint::Recolor {
                                edge_basis: Some(from.to_srgba_unmultiplied()),
                                alpha: self.color_alpha,
                                from: if self.recolor_once {
                                    self.recolor_sample
                                        .unwrap_or_else(|| from.to_srgba_unmultiplied())
                                } else {
                                    from.to_srgba_unmultiplied()
                                },
                                to: drawing_color.to_srgba_unmultiplied(),
                                tolerance: paint_rust::selection::tolerance_threshold(
                                    self.color_tolerance,
                                ),
                            }
                        };
                        self.document.paint_segment(
                            self.last_point.or(self.shape_origin).unwrap_or(point),
                            point,
                            self.brush_size,
                            paint,
                            self.brush_options,
                        );
                        self.texture_dirty = true;
                    }
                    let shape = match self.tool {
                        Tool::Line => Some(Shape::Line),
                        Tool::Shapes => Some(self.shape_kind),
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
                        self.request_gesture_preview(Effect::Shape(
                            paint_rust::shapes::Parameters {
                                shape,
                                from: origin,
                                to: end,
                                style: ShapeStyle {
                                    paint_mode: self
                                        .brush_options
                                        .blend
                                        .then_some(self.brush_options.blend_mode),
                                    antialias: self.shape_antialias,
                                    outline: (self.shape_filled && self.shape_outline)
                                        .then_some(drawing_color.to_srgba_unmultiplied()),
                                    color: if self.shape_filled && self.shape_outline {
                                        if drawing_button == egui::PointerButton::Secondary {
                                            self.color
                                        } else {
                                            self.secondary_color
                                        }
                                    } else {
                                        drawing_color
                                    }
                                    .to_srgba_unmultiplied(),
                                    width: self.brush_size,
                                    filled: self.shape_filled,
                                },
                            },
                        ));
                    }
                    if self.tool == Tool::Gradient {
                        let mut start = drawing_color.to_srgba_unmultiplied();
                        let mut end = if drawing_button == egui::PointerButton::Secondary {
                            self.color
                        } else {
                            self.secondary_color
                        }
                        .to_srgba_unmultiplied();
                        if self.gradient_mode == paint_rust::gradient::GradientMode::Transparency {
                            let reverse = drawing_button == egui::PointerButton::Secondary;
                            start[3] = if reverse { 0 } else { 255 };
                            end[3] = if reverse { 255 } else { 0 };
                        }
                        let effect = Effect::Gradient(paint_rust::gradient::Gradient {
                            from: self.shape_origin.unwrap_or(point),
                            to: point,
                            style: GradientStyle {
                                blend_mode: self.brush_options.blend_mode,
                                start,
                                end,
                                shape: self.gradient_shape,
                                mode: self.gradient_mode,
                            },
                        });
                        self.request_gesture_preview(effect);
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
                        if self.move_selection_only {
                            self.document.preview_move(dx, dy, false, true);
                            self.texture_dirty = true;
                        } else {
                            self.request_gesture_preview(Effect::Move(
                                paint_rust::movement::Parameters {
                                    dx,
                                    dy,
                                    copy: ui.input(|i| i.modifiers.command),
                                },
                            ));
                        }
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
                        let selection = if self.ellipse_selection {
                            Selection::elliptical(x, y, right - x, bottom - y)
                        } else {
                            Selection {
                                mask: None,
                                ellipse: None,
                                x,
                                y,
                                width: right - x,
                                height: bottom - y,
                            }
                        };
                        self.document.set_selection(Some(selection.clone()));
                        ui.ctx().request_repaint();
                    }
                    self.last_point = Some(point);
                    if !ui.input(|i| i.pointer.button_down(drawing_button)) && !self.retain_shape()
                    {
                        self.finish_stroke();
                    }
                }
            });
        self.canvas_offset = scroll.state.offset;
        if let Some((zoom, offset)) = requested_view {
            self.zoom = zoom;
            self.canvas_offset = offset;
            ui.ctx().request_repaint();
        }
        self.canvas_viewport = scroll.inner_rect;
    }

    fn tool_options(&mut self, ui: &mut egui::Ui) {
        if matches!(
            self.tool,
            Tool::Brush
                | Tool::Eraser
                | Tool::Clone
                | Tool::Recolor
                | Tool::Line
                | Tool::Shapes
                | Tool::Rectangle
                | Tool::Ellipse
        ) {
            toolbar::slider(
                ui,
                egui::Slider::new(&mut self.brush_size, 1.0..=128.0).suffix(" px"),
                "Brush size",
            );
        }
        if matches!(
            self.tool,
            Tool::Brush | Tool::Eraser | Tool::Clone | Tool::Recolor
        ) {
            ui.add_enabled_ui(self.brush_options.antialias, |ui| {
                toolbar::slider(
                    ui,
                    egui::Slider::new(&mut self.brush_options.hardness, 0.0..=1.0),
                    "Hardness",
                );
            })
            .response
            .on_disabled_hover_text("Enable antialiasing to use a soft brush edge");
            toolbar::slider(
                ui,
                egui::Slider::new(&mut self.brush_options.opacity, 0.0..=1.0),
                "Stroke opacity",
            );
            ui.checkbox(&mut self.brush_options.antialias, "Antialias");
            ui.menu_button("Stroke options", |ui| {
            toolbar::slider(
                ui,
                egui::Slider::new(&mut self.brush_options.spacing, 0.0..=1000.0)
                    .max_decimals(1)
                    .suffix("%")
                    .logarithmic(true),
                "Dab spacing",
            )
            .on_hover_text("Percentage of brush diameter; 0% draws a continuous stroke");
            toolbar::slider(ui, egui::Slider::new(&mut self.brush_options.smoothing, 0.0..=256.0).suffix(" px").logarithmic(true), "Stabilizer")
                .on_hover_text("Response distance in image pixels; 0 disables smoothing. The stroke reaches the release point when finished.");
                ui.small("Settings are saved for future strokes.");
            }).response.on_hover_text("Dab spacing and stroke stabilization");
            if matches!(self.tool, Tool::Brush | Tool::Clone) {
                ui.checkbox(&mut self.brush_options.blend, "Blend color");
                ui.menu_button(
                    format!("Paint mode: {}", self.brush_options.blend_mode.name()),
                    |ui| {
                        for mode in BlendMode::ALL {
                            if ui
                                .selectable_value(
                                    &mut self.brush_options.blend_mode,
                                    mode,
                                    mode.name(),
                                )
                                .clicked()
                            {
                                ui.close();
                            }
                        }
                    },
                )
                .response
                .on_hover_text("Applied when Blend color is enabled");
            }
        }
        if self.tool == Tool::Clone {
            if ui
                .checkbox(&mut self.clone_aligned, "Aligned source")
                .changed()
            {
                self.clone_offset = None;
            }
            ui.checkbox(&mut self.clone_merged, "Sample merged image");
            if let Some(layer) = self
                .document
                .layers()
                .iter()
                .find(|layer| Some(layer.session_id()) == self.clone_layer)
            {
                ui.small(format!("Source layer: {}", layer.name));
            }
            ui.small("Ctrl+click: set source · Switch layer and drag to clone");
            if !self
                .clone_anchor
                .is_some_and(|(id, _)| id == self.active_document_id)
            {
                ui.label("Set a source before painting.");
            }
        }
        if self.tool == Tool::Recolor {
            egui::ComboBox::from_label("Recolor sampling")
                .selected_text(if self.recolor_once {
                    "Sample once per stroke"
                } else {
                    "Primary/secondary color"
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.recolor_once, true, "Sample once per stroke");
                    ui.selectable_value(&mut self.recolor_once, false, "Primary/secondary color");
                });
            ui.add(
                egui::Slider::new(&mut self.color_tolerance, 0..=100)
                    .suffix("%")
                    .text("Color tolerance"),
            );
            ui.small(if self.recolor_once {
                "First clicked color → primary; right button uses secondary."
            } else {
                "Left: secondary → primary · Right: primary → secondary"
            });
        }
        if self.tool == Tool::Pencil {
            ui.label("Pencil: 1 px");
        }
        if self.tool == Tool::Gradient {
            egui::ComboBox::from_label("Gradient shape")
                .selected_text(self.gradient_shape.label())
                .show_ui(ui, |ui| {
                    for shape in paint_rust::gradient::GradientShape::ALL {
                        ui.selectable_value(&mut self.gradient_shape, shape, shape.label());
                    }
                });
            egui::ComboBox::from_label("Gradient mode")
                .selected_text(self.gradient_mode.label())
                .show_ui(ui, |ui| {
                    for mode in paint_rust::gradient::GradientMode::ALL {
                        ui.selectable_value(&mut self.gradient_mode, mode, mode.label());
                    }
                });
            ui.small("Right drag reverses colors or transparency");
        }
        if matches!(self.tool, Tool::ColorSelect | Tool::Fill | Tool::Recolor) {
            egui::ComboBox::from_label("Tolerance alpha")
                .selected_text(self.color_alpha.label())
                .show_ui(ui, |ui| {
                    for mode in [
                        paint_rust::selection::AlphaMatch::Premultiplied,
                        paint_rust::selection::AlphaMatch::Straight,
                    ] {
                        ui.selectable_value(&mut self.color_alpha, mode, mode.label());
                    }
                });
        }
        if matches!(self.tool, Tool::ColorSelect | Tool::Fill) {
            ui.add(
                egui::Slider::new(&mut self.color_tolerance, 0..=100)
                    .suffix("%")
                    .text("Tolerance"),
            );
            ui.checkbox(&mut self.color_contiguous, "Contiguous");
            ui.checkbox(&mut self.color_sample_merged, "Sample merged image");
        }
        if matches!(
            self.tool,
            Tool::Select | Tool::Polygon | Tool::ColorSelect | Tool::Lasso
        ) {
            if self.tool == Tool::Select {
                ui.checkbox(&mut self.ellipse_selection, "Elliptical selection");
            } else if self.tool == Tool::Lasso {
                ui.small("Drag to outline an area. Release closes the outline; Escape cancels.");
            } else if self.tool == Tool::Polygon {
                ui.small("Click points. Enter or double-click finishes. Backspace removes a point; Escape cancels.");
            }
            egui::ComboBox::from_label("Combine")
                .selected_text(self.selection_mode.label())
                .show_ui(ui, |ui| {
                    for mode in [
                        CombineMode::Replace,
                        CombineMode::Add,
                        CombineMode::Subtract,
                        CombineMode::Intersect,
                        CombineMode::Xor,
                    ] {
                        ui.selectable_value(&mut self.selection_mode, mode, mode.label());
                    }
                });
        }
        if self.tool == Tool::Move {
            ui.checkbox(&mut self.move_selection_only, "Move selection only");
            if self.move_selection_only
                && ui
                    .add_enabled(
                        self.document.selection().is_some(),
                        egui::Button::new("Transform boundary…"),
                    )
                    .clicked()
            {
                self.begin_selection_transform(&ui.ctx().clone());
            }

            ui.small("Drag: Shift constrains direction; Ctrl copies pixels.");
            ui.small("Arrows: move 1 px. Shift+arrows: move 10 px.");
        }
        if self.tool == Tool::Shapes {
            egui::ComboBox::from_label("Shape")
                .selected_text(self.shape_kind.label())
                .show_ui(ui, |ui| {
                    for category in [
                        "Basic",
                        "Polygons and stars",
                        "Arrows",
                        "Callouts",
                        "Symbols",
                    ] {
                        ui.strong(category);
                        for shape in Shape::ALL
                            .into_iter()
                            .filter(|shape| shape.category() == category)
                        {
                            ui.selectable_value(&mut self.shape_kind, shape, shape.label());
                        }
                        ui.separator();
                    }
                });
        }
        if matches!(
            self.tool,
            Tool::Line | Tool::Rectangle | Tool::Ellipse | Tool::Shapes
        ) {
            ui.checkbox(&mut self.shape_antialias, "Antialias shape");
            ui.checkbox(&mut self.keep_shape_editable, "Edit after drawing");
        }
        if matches!(
            self.tool,
            Tool::Line
                | Tool::Rectangle
                | Tool::Ellipse
                | Tool::Shapes
                | Tool::Gradient
                | Tool::Pencil
                | Tool::Fill
        ) {
            if self.tool != Tool::Gradient {
                ui.checkbox(&mut self.brush_options.blend, "Blend color");
            }
            egui::ComboBox::from_label("Paint mode")
                .selected_text(self.brush_options.blend_mode.name())
                .show_ui(ui, |ui| {
                    for mode in BlendMode::ALL {
                        ui.selectable_value(&mut self.brush_options.blend_mode, mode, mode.name());
                    }
                });
            ui.small(if self.tool == Tool::Gradient {
                "Paint mode applies to Blend colors."
            } else {
                "Paint mode applies when Blend color is enabled."
            });
        }
        if matches!(self.tool, Tool::Rectangle | Tool::Ellipse | Tool::Shapes) {
            ui.checkbox(&mut self.shape_filled, "Filled shape");
            if self.shape_filled {
                ui.checkbox(&mut self.shape_outline, "Outline and fill");
            }
            if self.shape_filled && self.shape_outline {
                ui.small("Primary outline · Secondary fill · Right drag swaps colors");
            }
            ui.small("Hold Shift for equal sides.");
        }
        if self.tool == Tool::Line {
            ui.small("Hold Shift for 45° angles.");
        }
        if matches!(
            self.tool,
            Tool::Brush
                | Tool::Pencil
                | Tool::Fill
                | Tool::Line
                | Tool::Shapes
                | Tool::Rectangle
                | Tool::Ellipse
                | Tool::Picker
        ) {
            ui.small("Left: primary · Right: secondary · X: swap");
        }
        if self.tool == Tool::Eraser {
            ui.small("Drag to erase · Escape cancels");
        }
        if self.tool == Tool::Zoom {
            ui.small("Left: zoom in · Right: zoom out · Drag: fit region");
        }
        if self.tool == Tool::Pan {
            ui.small("Drag to pan · Hold Space to pan from another tool");
        }
        if self.tool == Tool::Text {
            ui.small("Left: primary text · Right: secondary text");
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        self.show_export(ctx);
        if self.file_job.is_some() {
            egui::Window::new("Working")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(&self.status);
                    });
                    if let Some(cancel) = self.file_job.as_ref().and_then(|job| job.cancel.as_ref())
                    {
                        use std::sync::atomic::Ordering;
                        if ui
                            .add_enabled(
                                !cancel.load(Ordering::Relaxed),
                                egui::Button::new("Cancel"),
                            )
                            .clicked()
                        {
                            cancel.store(true, Ordering::Relaxed);
                            self.status = "Cancelling operation…".into();
                        }
                    }
                });
            return;
        }
        if !self.open_errors.is_empty() && self.open_queue.is_empty() {
            let mut dismiss = false;
            egui::Window::new("Files that could not be opened")
                .collapsible(false)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for error in &self.open_errors {
                                ui.label(error);
                            }
                        });
                    dismiss = ui.button("Dismiss").clicked();
                });
            if dismiss {
                self.open_errors.clear();
            }
        }
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
                            self.after_save = Some(action);
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
                    let label = ui.label("Name");
                    ui.text_edit_singleline(&mut name).labelled_by(label.id);
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
            let mut keep = !ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
            egui::Window::new(if dialog.canvas {
                "Canvas size"
            } else {
                "Resize image"
            })
            .collapsible(false)
            .resizable(false)
            .vscroll(false)
            .max_height((ctx.content_rect().height()-100.0).max(180.0))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-200.0).max(100.0)).show(ui,|ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut dialog.percentage,false,"Absolute size");
                    if ui.selectable_value(&mut dialog.percentage,true,"Percentage").changed() {
                        dialog.percent=100.0*f64::from(dialog.width)/f64::from(dialog.original_size.0);
                        dialog.apply_percentage();
                    }
                });
                if dialog.percentage {
                    let max_percent=1638400.0 / f64::from(dialog.original_size.0.max(dialog.original_size.1));
                    if ui.add(egui::DragValue::new(&mut dialog.percent).range(0.01..=max_percent).speed(1.0).suffix(" %")).changed() {
                        dialog.apply_percentage();
                    }
                }
                ui.add_enabled_ui(!dialog.percentage,|ui| {
                egui::ComboBox::from_label("Measurement units").selected_text(dialog.units.label()).show_ui(ui,|ui| {
                    for unit in paint_rust::resolution::Unit::ALL {ui.selectable_value(&mut dialog.units,unit,unit.label());}
                });
                for axis in 0..2 {
                    let factor=dialog.units.pixels_per_unit(if axis==0 {dialog.dpi.x}else{dialog.dpi.y});
                    let mut value=f64::from(if axis==0 {dialog.width}else{dialog.height})/factor;
                    ui.horizontal(|ui| {
                        ui.label(if axis==0 {"Width"}else{"Height"});
                        if ui.add(egui::DragValue::new(&mut value).range(1.0/factor..=16384.0/factor).speed(if dialog.units==paint_rust::resolution::Unit::Pixels {1.0}else{0.01}).suffix(format!(" {}",dialog.units.suffix()))).changed() {
                            let pixels=(value*factor).round().clamp(1.0,16384.0) as u32;
                            if axis==0 {dialog.width=pixels; if dialog.lock_ratio {dialog.height=(f64::from(pixels)/dialog.ratio).round().clamp(1.0,16384.0) as u32;}}
                            else {dialog.height=pixels; if dialog.lock_ratio {dialog.width=(f64::from(pixels)*dialog.ratio).round().clamp(1.0,16384.0) as u32;}}
                        }
                    });
                }
                });
                ui.label(format!("Pixel size: {} × {}",dialog.width,dialog.height));
                ui.horizontal_wrapped(|ui| {
                    if ui.add(egui::DragValue::new(&mut dialog.dpi.x).range(1.0..=10000.0).prefix("DPI X ").speed(1.0)).changed() && dialog.link_resolution {dialog.dpi.y=dialog.dpi.x;}
                    if ui.add(egui::DragValue::new(&mut dialog.dpi.y).range(1.0..=10000.0).prefix("DPI Y ").speed(1.0)).changed() && dialog.link_resolution {dialog.dpi.x=dialog.dpi.y;}
                });
                ui.checkbox(&mut dialog.link_resolution,"Link X/Y resolution");
                ui.small("Changing DPI keeps pixel dimensions and changes physical print size.");
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
                    let fills=["Transparent", "Primary color", "Secondary color", "White", "Black"];
                    egui::ComboBox::from_label("Fill new area").selected_text(fills[dialog.fill]).show_ui(ui,|ui| {
                        for (index,name) in fills.iter().enumerate() {ui.selectable_value(&mut dialog.fill,index,*name);}
                    });
                    ui.small("New area uses this fill on every layer; existing pixels stay unchanged.");
                } else {
                    egui::ComboBox::from_label("Resampling")
                        .selected_text(dialog.resampling.filter.name())
                        .show_ui(ui, |ui| { for filter in paint_rust::resample::Filter::ALL {
                            ui.selectable_value(&mut dialog.resampling.filter, filter, filter.name());
                        }});
                    ui.add_enabled(dialog.resampling.filter != paint_rust::resample::Filter::Nearest,
                        egui::Checkbox::new(&mut dialog.resampling.gamma_correct,"Gamma correction"));
                    ui.small("Gamma correction filters light in linear sRGB. Nearest neighbor copies exact pixels.");
                }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() && self.apply_resize_dialog(&dialog) {
                        keep = false;
                    }
                    if ui.button("Cancel").clicked() {
                        keep = false;
                    }
                });
                if self.status.starts_with("Could not resize") {
                    ui.colored_label(Color32::LIGHT_RED, &self.status);
                }
            });
            if keep
                && !ctx.wants_keyboard_input()
                && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter))
                && self.apply_resize_dialog(&dialog)
            {
                keep = false;
            }
            if keep {
                self.resize_dialog = Some(dialog);
            }
        }
        if let Some(mut dialog) = self.text_dialog.take() {
            let mut keep = true;
            let mut font_path = None;
            let mut font_record = None;
            let mut discover_fonts = false;
            if let Some(pixels) = dialog.preview.poll() {
                match self.document.publish_preview(pixels) {
                    Ok(()) => self.texture_dirty = true,
                    Err(error) => self.status = error,
                }
            }
            let cancel = ctx.input(|i| i.key_pressed(Key::Escape));
            if cancel {
                self.document.cancel();
                self.texture_dirty = true;
                keep = false;
            } else {
                egui::Window::new("Text")
                    .vscroll(false)
                    .max_height((ctx.content_rect().height() - 80.0).max(180.0))
                    .collapsible(false)
                    .resizable(true)
                    .default_width(320.0)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height((ctx.content_rect().height() - 220.0).max(100.0))
                            .show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(format!("Font: {}", dialog.font_name));
                                    if ui
                                        .add_enabled(
                                            self.file_job.is_none(),
                                            egui::Button::new("Load font…"),
                                        )
                                        .clicked()
                                    {
                                        font_path = rfd::FileDialog::new()
                                            .add_filter("Font", &["ttf", "otf", "ttc"])
                                            .pick_file();
                                    }
                                });
                                let fallback_changed = ui.checkbox(&mut dialog.layout.automatic_fallback, "Use fallback fonts for missing characters").changed();
                                ui.small("Fallback uses installed outline fonts. Color emoji are not supported.");
                                egui::CollapsingHeader::new("Installed fonts").show(ui, |ui| {
                                    discover_fonts = ui
                                        .add_enabled(
                                            self.file_job.is_none(),
                                            egui::Button::new(if self.system_fonts.is_empty() {
                                                "Find installed fonts"
                                            } else {
                                                "Refresh fonts"
                                            }),
                                        )
                                        .clicked();
                                    let label = ui.label("Search family or style");
                                    ui.text_edit_singleline(&mut self.font_search)
                                        .labelled_by(label.id);
                                    let query = self.font_search.to_lowercase();
                                    let matching: Vec<_> = self
                                        .system_fonts
                                        .iter()
                                        .filter(|font| {
                                            font.family.to_lowercase().contains(&query)
                                                || font.style.to_lowercase().contains(&query)
                                        })
                                        .collect();
                                    egui::ScrollArea::vertical()
                                        .id_salt("installed-font-list")
                                        .max_height(150.0)
                                        .show(ui, |ui| {
                                            for record in matching.iter().take(200) {
                                                if ui
                                                    .add_enabled(
                                                        self.file_job.is_none(),
                                                        egui::Button::new(record.label()),
                                                    )
                                                    .on_hover_text(
                                                        record.path.display().to_string(),
                                                    )
                                                    .clicked()
                                                {
                                                    font_record = Some((*record).clone());
                                                }
                                            }
                                        });
                                    if matching.len() > 200 {
                                        ui.small(format!(
                                            "Showing 200 of {} matches; narrow the search.",
                                            matching.len()
                                        ));
                                    }
                                });
                                ui.small("If a character is still missing, install an outline font for its language.");
                                let label = ui.label("Text content");
                                let response = ui
                                    .add(
                                        egui::TextEdit::multiline(&mut dialog.content)
                                            .desired_rows(4)
                                            .char_limit(4096),
                                    )
                                    .labelled_by(label.id);
                                if dialog.focus {
                                    response.request_focus();
                                    dialog.focus = false;
                                }
                                let mut changed = response.changed() || fallback_changed;
                                let axes = paint_rust::text::variations::axes(
                                    &dialog.font,
                                    dialog.layout.font_index,
                                );
                                if !axes.is_empty() {
                                    egui::CollapsingHeader::new("Font variations").show(ui, |ui| {
                                        for axis in axes {
                                            let mut value = dialog
                                                .layout
                                                .font_axes
                                                .get(axis.tag)
                                                .unwrap_or(axis.default_value);
                                            let label = axis.name.unwrap_or_else(|| {
                                                String::from_utf8_lossy(&axis.tag).into_owned()
                                            });
                                            if ui
                                                .add(
                                                    egui::Slider::new(
                                                        &mut value,
                                                        axis.min_value..=axis.max_value,
                                                    )
                                                    .text(label),
                                                )
                                                .changed()
                                            {
                                                changed |=
                                                    dialog.layout.font_axes.set(axis.tag, value);
                                            }
                                        }
                                        if ui.button("Reset font variations").clicked() {
                                            dialog.layout.font_axes = Default::default();
                                            changed = true;
                                        }
                                    });
                                }
                                changed |= ui
                                    .add(
                                        egui::Slider::new(&mut dialog.size, 4.0..=256.0)
                                            .text("Size (px)"),
                                    )
                                    .changed();
                                egui::ComboBox::from_label("Text paint mode")
                                    .selected_text(dialog.layout.blend_mode.name())
                                    .show_ui(ui, |ui| {
                                        for mode in BlendMode::ALL {
                                            changed |= ui
                                                .selectable_value(
                                                    &mut dialog.layout.blend_mode,
                                                    mode,
                                                    mode.name(),
                                                )
                                                .changed();
                                        }
                                    });
                                egui::ComboBox::from_label("Alignment")
                                    .selected_text(dialog.layout.alignment.label())
                                    .show_ui(ui, |ui| {
                                        for alignment in [
                                            paint_rust::text::Alignment::Left,
                                            paint_rust::text::Alignment::Center,
                                            paint_rust::text::Alignment::Right,
                                        ] {
                                            changed |= ui
                                                .selectable_value(
                                                    &mut dialog.layout.alignment,
                                                    alignment,
                                                    alignment.label(),
                                                )
                                                .changed();
                                        }
                                    });
                                changed |= ui
                                    .add(
                                        egui::Slider::new(
                                            &mut dialog.layout.letter_spacing,
                                            -32.0..=128.0,
                                        )
                                        .text("Letter spacing (px)"),
                                    )
                                    .changed();
                                changed |= ui
                                    .add(
                                        egui::Slider::new(
                                            &mut dialog.layout.line_spacing,
                                            0.5..=3.0,
                                        )
                                        .text("Line spacing"),
                                    )
                                    .changed();
                                changed |= ui
                                    .checkbox(&mut dialog.layout.antialias, "Antialias text")
                                    .changed();
                                changed |= ui
                                    .checkbox(&mut dialog.layout.underline, "Underline")
                                    .changed();
                                changed |= ui
                                    .checkbox(&mut dialog.layout.strikethrough, "Strikethrough")
                                    .changed();
                                let label = ui.label("Text color");
                                changed |= dialog.color.edit(ui).labelled_by(label.id).changed();
                                if changed {
                                    dialog.preview.request_text(crate::preview::TextInput {
                                        layout: dialog.layout,
                                        font: dialog.font.clone(),
                                        content: dialog.content.clone(),
                                        origin: dialog.origin,
                                        size: dialog.size,
                                        color: dialog.color.to_srgba_unmultiplied(),
                                    });
                                }
                                if !dialog.preview.ready {
                                    ui.label("Rendering preview…");
                                }
                            });
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    dialog.preview.ready && self.file_job.is_none(),
                                    egui::Button::new("Apply"),
                                )
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
                        ui.small("Text becomes pixels on the active layer when applied.");
                    });
            }
            if keep {
                self.text_dialog = Some(dialog);
                if let Some(path) = font_path {
                    self.load_text_font(path);
                } else if let Some(record) = font_record {
                    self.load_installed_font(record);
                } else if discover_fonts {
                    self.discover_fonts();
                }
            } else {
                self.last_point = None;
                self.shape_origin = None;
            }
        }
        if let Some(mut dialog) = self.effect_dialog.take() {
            let mut keep = true;
            if let Some(pixels) = dialog.preview.poll() {
                match self.document.publish_preview(pixels) {
                    Ok(()) => {
                        self.texture_dirty = true;
                        if dialog.histogram.is_some() {
                            dialog.output_histogram = Some(histogram::Histogram::new(
                                ctx.clone(),
                                self.document.shared_pixels(),
                                self.document.width,
                                self.document.selection(),
                            ));
                        }
                    }
                    Err(error) => self.status = error,
                }
            }
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.document.cancel();
                self.texture_dirty = true;
                keep = false;
            } else {
                egui::Window::new(dialog.effect.name())
                    .vscroll(true)
                    .max_height((ctx.content_rect().height() - 80.0).max(180.0))
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        let mut changed = false;
                        match &mut dialog.effect {
                            Effect::FreeTransform(parameters) => {
                                changed |= ui.add(egui::Slider::new(&mut parameters.angle, -180.0..=180.0).text("Angle (degrees)")).changed();
                                for (label, scale) in [("Horizontal scale", &mut parameters.scale_x), ("Vertical scale", &mut parameters.scale_y)] {
                                    changed |= ui.add(egui::Slider::new(scale, 0.01..=4.0).logarithmic(true)
                                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)).text(label)).changed();
                                }
                                ui.horizontal(|ui| {
                                    let label = ui.label("Offset X");
                                    changed |= ui.add(egui::DragValue::new(&mut parameters.offset_x).range(-16384.0..=16384.0).suffix(" px")).labelled_by(label.id).changed();
                                    let label = ui.label("Offset Y");
                                    changed |= ui.add(egui::DragValue::new(&mut parameters.offset_y).range(-16384.0..=16384.0).suffix(" px")).labelled_by(label.id).changed();
                                });
                                changed |= ui.checkbox(&mut parameters.flip_x, "Flip horizontally").changed();
                                changed |= ui.checkbox(&mut parameters.flip_y, "Flip vertically").changed();
                                changed |= ui.checkbox(&mut parameters.copy, "Keep source pixels").changed();
                                changed |= ui.checkbox(&mut parameters.smooth, "Smooth sampling").changed();
                                ui.small("Transforms around the selection center. Pixels outside the canvas are clipped.");
                                ui.small("Drag the box to move, corners to scale, or the blue circle to rotate. Shift constrains movement, preserves scale ratio, or snaps rotation.");
                            }
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
                            Effect::Exposure { stops, offset, gamma } => {
                                changed |= ui.add(egui::Slider::new(stops,-10.0..=10.0).text("Exposure (EV)")).changed();
                                changed |= ui.add(egui::Slider::new(offset,-1.0..=1.0).text("Linear offset")).changed();
                                changed |= ui.add(egui::Slider::new(gamma,0.1..=10.0).logarithmic(true).text("Gamma")).changed();
                            }
                            Effect::Curves(parameters) => {
                                changed |= curves::controls(ui, parameters, &mut self.levels_channel);
                            }
                            Effect::Levels(parameters) => {
                                let names=["RGB","Red","Green","Blue"];
                                egui::ComboBox::from_label("Channel").selected_text(names[self.levels_channel]).show_ui(ui, |ui| {
                                    for (index,name) in names.iter().enumerate() { ui.selectable_value(&mut self.levels_channel,index,*name); }
                                });
                                let channel=&mut parameters.channels[self.levels_channel];
                                changed |= ui.add(egui::Slider::new(&mut channel.input_black,0..=channel.input_white-1).text("Input black")).changed();
                                changed |= ui.add(egui::Slider::new(&mut channel.input_white,channel.input_black+1..=255).text("Input white")).changed();
                                changed |= ui.add(egui::Slider::new(&mut channel.gamma,0.1..=10.0).logarithmic(true).text("Gamma")).changed();
                                changed |= ui.add(egui::Slider::new(&mut channel.output_black,0..=255).text("Output black")).changed();
                                changed |= ui.add(egui::Slider::new(&mut channel.output_white,0..=255).text("Output white")).changed();
                                ui.horizontal(|ui| {
                                    if ui.button("Reset channel").clicked() { parameters.channels[self.levels_channel]=Default::default(); changed=true; }
                                    if ui.button("Reset all").clicked() { *parameters=Default::default(); changed=true; }
                                });
                            }
                            Effect::AutoLevels(clip) => {
                                changed |= ui.add(egui::Slider::new(clip,0.0..=5.0).text("Clip each tail (%)")).changed();
                                ui.small("Uses selected, nontransparent pixels to estimate each channel's range.");
                            }
                            Effect::HueSaturation { hue, saturation, lightness } => {
                                changed |= ui.add(egui::Slider::new(hue,-180..=180).text("Hue (degrees)")).changed();
                                changed |= ui.add(egui::Slider::new(saturation,0.0..=2.0).text("Saturation")).changed();
                                changed |= ui.add(egui::Slider::new(lightness,-100..=100).text("Lightness")).changed();
                            }
                            Effect::Saturation(amount) => {
                                changed |= ui
                                    .add(egui::Slider::new(amount, 0.0..=2.0).text("Saturation"))
                                    .changed();
                            }
                            Effect::Pixelate(size) => {
                                changed |= ui
                                    .add(egui::Slider::new(size, 1..=128).text("Block size (px)"))
                                    .changed();
                            }
                            Effect::Sharpen(amount) => {
                                changed |= ui.add(egui::Slider::new(amount, 0.0..=3.0).text("Amount")).changed();
                            }
                            Effect::Posterize(levels) => {
                                changed |= ui
                                    .add(
                                        egui::Slider::new(levels, 2..=32)
                                            .text("Levels per channel"),
                                    )
                                    .changed();
                            }
                            Effect::Bulge(amount) => {changed |= ui.add(egui::Slider::new(amount,-100.0..=100.0).text("Amount")).changed();}
                            Effect::Twist(angle) => {changed |= ui.add(egui::Slider::new(angle,-720.0..=720.0).text("Angle (degrees)")).changed();}
                            Effect::FrostedGlass { radius,seed } => {
                                changed |= ui.add(egui::Slider::new(radius,0..=100).text("Scatter radius (px)")).changed();
                                let label=ui.label("Seed");changed |= ui.add(egui::DragValue::new(seed)).labelled_by(label.id).changed();
                            }
                            Effect::Crystalize { size,seed } => {
                                changed |= ui.add(egui::Slider::new(size,1..=200).text("Cell size (px)")).changed();
                                let label=ui.label("Seed");changed |= ui.add(egui::DragValue::new(seed)).labelled_by(label.id).changed();
                            }
                            Effect::Vignette { amount,roundness } => {
                                changed |= ui.add(egui::Slider::new(amount,0.0..=1.0).text("Amount")).changed();
                                changed |= ui.add(egui::Slider::new(roundness,0.0..=1.0).text("Clear center radius")).changed();
                            }
                            Effect::RedEye(threshold) => {
                                changed |= ui.add(egui::Slider::new(threshold,1.0..=3.0).text("Red dominance threshold")).changed();
                                ui.small("Select the eye area first to protect other red objects.");
                            }
                            Effect::AddNoise { amount,coverage,monochrome,seed } => {
                                changed |= ui.add(egui::Slider::new(amount,0..=100).text("Intensity")).changed();
                                changed |= ui.add(egui::Slider::new(coverage,0..=100).text("Coverage (%)")).changed();
                                changed |= ui.checkbox(monochrome,"Monochrome").changed();
                                let label=ui.label("Seed");
                                changed |= ui.add(egui::DragValue::new(seed)).labelled_by(label.id).changed();
                                if ui.button("New seed").clicked() { *seed=seed.wrapping_add(1);changed=true; }
                            }
                            Effect::Median { radius,percentile } => {
                                changed |= ui.add(egui::Slider::new(radius,0..=10).text("Radius (px)")).changed();
                                changed |= ui.add(egui::Slider::new(percentile,0..=100).text("Percentile")).changed();
                            }
                            Effect::Emboss(angle) => {
                                changed |= ui.add(egui::Slider::new(angle,-180.0..=180.0).text("Angle (degrees)")).changed();
                            }
                            Effect::TemperatureTint { temperature,tint } => {
                                changed |= ui.add(egui::Slider::new(temperature,-100..=100).text("Temperature")).changed();
                                changed |= ui.add(egui::Slider::new(tint,-100..=100).text("Tint")).changed();
                            }
                            Effect::HighlightsShadows { highlights,shadows } => {
                                changed |= ui.add(egui::Slider::new(shadows,-100..=100).text("Shadows")).changed();
                                changed |= ui.add(egui::Slider::new(highlights,-100..=100).text("Highlights")).changed();
                            }
                            Effect::SketchBlur {radius,percentile,smoothness,seed} => {
                                changed |= ui.add(egui::Slider::new(radius,0..=100).text("Radius (px)")).changed();
                                changed |= ui.add(egui::Slider::new(percentile,0..=100).text("Percentile")).changed();
                                changed |= ui.add(egui::Slider::new(smoothness,1..=5).text("Smoothness")).changed();
                                changed |= ui.add(egui::DragValue::new(seed).prefix("Seed ")).changed();
                                if ui.button("New seed").clicked() {*seed=seed.wrapping_add(1);changed=true;}
                            }
                            Effect::Straighten {angle,auto_crop} => {
                                changed |= ui.add(egui::Slider::new(angle,-45.0..=45.0).text("Angle (degrees)")).changed();
                                changed |= ui.checkbox(auto_crop,"Fill frame after rotation").changed();
                                ui.small("Rotates around the canvas center; image dimensions stay unchanged.");
                            }
                            Effect::BokehBlur {radius,gamma,quality} => {
                                changed |= ui.add(egui::Slider::new(radius,0.0..=100.0).text("Radius (px)")).changed();
                                changed |= ui.add(egui::Slider::new(gamma,0.1..=5.0).text("Gamma boost")).changed();
                                changed |= ui.add(egui::Slider::new(quality,1..=5).text("Quality")).changed();
                                ui.small("Higher quality takes longer to render.");
                            }
                            Effect::Quantize(colors) => {
                                changed |= ui.add(egui::Slider::new(colors,2..=256).text("Maximum colors")).changed();
                                ui.small("Reduces RGB colors while retaining opacity. No dithering.");
                            }
                            Effect::OilPainting {radius,bins} => {
                                changed |= ui.add(egui::Slider::new(radius,0..=10).text("Radius (px)")).changed();
                                changed |= ui.add(egui::Slider::new(bins,2..=64).text("Tonal bins")).changed();
                            }
                            Effect::PencilSketch {radius,strength} => {
                                changed |= ui.add(egui::Slider::new(radius,1..=64).text("Detail radius (px)")).changed();
                                changed |= ui.add(egui::Slider::new(strength,0.0..=1.0).text("Strength")).changed();
                            }
                            Effect::InkSketch {threshold,color} => {
                                changed |= ui.add(egui::Slider::new(threshold,0..=255).text("Edge threshold")).changed();
                                changed |= ui.add(egui::Slider::new(color,0.0..=1.0).text("Color amount")).changed();
                            }
                            Effect::ReduceNoise {radius,strength} => {
                                changed |= ui.add(egui::Slider::new(radius,0..=10).text("Radius (px)")).changed();
                                changed |= ui.add(egui::Slider::new(strength,0.0..=1.0).text("Strength")).changed();
                            }
                            Effect::Relief {angle,strength} => {
                                changed |= ui.add(egui::Slider::new(angle,-180.0..=180.0).text("Angle (degrees)")).changed();
                                changed |= ui.add(egui::Slider::new(strength,0.0..=3.0).text("Strength")).changed();
                            }
                            Effect::Outline {radius,intensity} => {
                                changed |= ui.add(egui::Slider::new(radius,1..=10).text("Radius (px)")).changed();
                                changed |= ui.add(egui::Slider::new(intensity,0.0..=4.0).text("Intensity")).changed();
                            }
                            Effect::Morphology {radius,operation} => {
                                changed |= ui.add(egui::Slider::new(radius,0..=64).text("Radius (px)")).changed();
                                egui::ComboBox::from_label("Operation").selected_text(operation.label()).show_ui(ui,|ui| {
                                    for value in paint_rust::effects::MorphologyOperation::ALL {
                                        changed |= ui.selectable_value(operation,value,value.label()).changed();
                                    }
                                });
                                ui.small("Square neighborhood; color and opacity change together.");
                            }
                            Effect::Dents {scale,amount,seed} => {
                                changed |= ui.add(egui::Slider::new(scale,1.0..=1000.0).logarithmic(true).text("Scale (px)")).changed();
                                changed |= ui.add(egui::Slider::new(amount,0.0..=200.0).text("Displacement (px)")).changed();
                                changed |= ui.add(egui::DragValue::new(seed).prefix("Seed ")).changed();
                                if ui.button("New seed").clicked() {*seed=seed.wrapping_add(1);changed=true;}
                            }
                            Effect::PolarInversion {amount,center} => {
                                changed |= ui.add(egui::Slider::new(amount,-1.0..=1.0).text("Amount")).changed();
                                changed |= ui.add(egui::Slider::new(&mut center[0],0.0..=1.0).text("Center X")).changed();
                                changed |= ui.add(egui::Slider::new(&mut center[1],0.0..=1.0).text("Center Y")).changed();
                            }
                            Effect::TileReflection {size,amount,angle} => {
                                changed |= ui.add(egui::Slider::new(size,1.0..=500.0).text("Tile size (px)")).changed();
                                changed |= ui.add(egui::Slider::new(amount,-100.0..=100.0).text("Displacement (px)")).changed();
                                changed |= ui.add(egui::Slider::new(angle,-180.0..=180.0).text("Angle (degrees)")).changed();
                            }
                            Effect::Clouds(settings) | Effect::Turbulence(settings) => {
                                changed |= ui.add(egui::Slider::new(&mut settings.scale,1.0..=1000.0).logarithmic(true).text("Scale (px)")).changed();
                                changed |= ui.add(egui::Slider::new(&mut settings.roughness,0.0..=1.0).text("Roughness")).changed();
                                changed |= ui.add(egui::DragValue::new(&mut settings.seed).prefix("Seed ")).changed();
                                if ui.button("New seed").clicked() { settings.seed=settings.seed.wrapping_add(1);changed=true; }
                                changed |= render_controls::colors(ui,&mut settings.colors);
                                ui.small("Replaces selected pixels with a generated texture.");
                            }
                            Effect::Mandelbrot(settings) => {
                                changed |= render_controls::fractal(ui,settings,false);
                            }
                            Effect::Julia(settings) => {
                                changed |= render_controls::fractal(ui,settings,true);
                            }
                            Effect::Glow { radius, amount } => {
                                changed |= ui.add(egui::Slider::new(radius,0..=64).text("Radius (px)")).changed();
                                changed |= ui.add(egui::Slider::new(amount,0.0..=1.0).text("Amount")).changed();
                            }
                            Effect::SoftenPortrait { radius, softness, warmth } => {
                                changed |= ui.add(egui::Slider::new(radius,0..=64).text("Radius (px)")).changed();
                                changed |= ui.add(egui::Slider::new(softness,0.0..=1.0).text("Softness")).changed();
                                changed |= ui.add(egui::Slider::new(warmth,-1.0..=1.0).text("Warmth")).changed();
                            }
                            Effect::DropShadow { radius, offset, color } => {
                                changed |= ui.add(egui::Slider::new(radius,0..=64).text("Blur radius (px)")).changed();
                                changed |= ui.add(egui::Slider::new(&mut offset[0],-200..=200).text("Offset X (px)")).changed();
                                changed |= ui.add(egui::Slider::new(&mut offset[1],-200..=200).text("Offset Y (px)")).changed();
                                ui.label("Shadow color and opacity");
                                let mut rgba=Rgba::from_rgba_unmultiplied(color[0],color[1],color[2],color[3]);
                                if rgba.edit(ui).changed() { *color=rgba.to_srgba_unmultiplied(); changed=true; }
                                ui.small("The shadow is clipped to the canvas and selection.");
                            }
                            Effect::SurfaceBlur { radius, threshold } => {
                                changed |= ui.add(egui::Slider::new(radius,0..=10).text("Radius (px)")).changed();
                                changed |= ui.add(egui::Slider::new(threshold,1..=255).text("Color threshold")).changed();
                            }
                            Effect::RadialBlur { angle, center } => {
                                changed |= ui.add(egui::Slider::new(angle,0.0..=180.0).text("Angle (degrees)")).changed();
                                changed |= ui.add(egui::Slider::new(&mut center[0],0.0..=1.0).text("Center X")).changed();
                                changed |= ui.add(egui::Slider::new(&mut center[1],0.0..=1.0).text("Center Y")).changed();
                            }
                            Effect::ZoomBlur { amount, center } => {
                                changed |= ui.add(egui::Slider::new(amount,0.0..=1.0).text("Amount")).changed();
                                changed |= ui.add(egui::Slider::new(&mut center[0],0.0..=1.0).text("Center X")).changed();
                                changed |= ui.add(egui::Slider::new(&mut center[1],0.0..=1.0).text("Center Y")).changed();
                            }
                            Effect::FragmentBlur { distance, fragments, angle } => {
                                changed |= ui.add(egui::Slider::new(distance,0.0..=200.0).text("Distance (px)")).changed();
                                changed |= ui.add(egui::Slider::new(fragments,2..=32).text("Fragments")).changed();
                                changed |= ui.add(egui::Slider::new(angle,-180.0..=180.0).text("Angle (degrees)")).changed();
                            }
                            Effect::MotionBlur { distance,angle,centered } => {
                                changed |= ui.add(egui::Slider::new(distance,0..=200).text("Distance (px)")).changed();
                                changed |= ui.add(egui::Slider::new(angle,-180.0..=180.0).text("Angle (degrees)")).changed();
                                changed |= ui.checkbox(centered,"Centered").changed();
                            }
                            Effect::GaussianBlur(radius) | Effect::BoxBlur(radius) => {
                                changed |= ui
                                    .add(egui::Slider::new(radius, 0..=32).text("Radius (px)"))
                                    .changed();
                            }
                            _ => {
                                ui.label("Preview on the active layer");
                            }
                        }
                        let histogram_channel=if matches!(dialog.effect,Effect::Curves(parameters) if parameters.luminance) {4} else {self.levels_channel};
                        if let Some(histogram)=&mut dialog.histogram { histogram.show(ui,histogram_channel,false); }
                        if changed { dialog.output_histogram=None; dialog.preview.request(dialog.effect); }
                        if let Some(histogram)=&mut dialog.output_histogram { histogram.show(ui,histogram_channel,true); }
                        if !dialog.preview.ready {
                            ui.label("Rendering preview…");
                        }
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(dialog.preview.ready, egui::Button::new("Apply"))
                                .clicked()
                            {
                                self.apply_effect(dialog.effect);
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
            let mut recover_all = false;
            egui::Window::new("Recover unsaved work")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    if self.file_job.is_some() {
                        ui.disable();
                    }
                    ui.label("An earlier session left unsaved image data.");
                    recover_all = ui
                        .add_enabled(
                            !self.recoverable.is_empty(),
                            egui::Button::new("Recover all into separate tabs"),
                        )
                        .clicked();
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            for (index, candidate) in self.recoverable.iter().enumerate() {
                                let minutes =
                                    candidate.modified.elapsed().unwrap_or_default().as_secs() / 60;
                                ui.horizontal(|ui| {
                                    ui.label(format!(
                                        "{} — last copied {minutes} min ago",
                                        candidate
                                            .context
                                            .as_ref()
                                            .and_then(|c| c.path.as_ref())
                                            .and_then(|p| p.file_name())
                                            .map(|p| p.to_string_lossy().into_owned())
                                            .unwrap_or_else(|| format!("Untitled {}", index + 1))
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
            if recover_all {
                self.start_recovery_batch();
            }
            if let Some((index, recover)) = chosen {
                if recover {
                    self.recover_candidate(index);
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
        self.show_new_image(ctx);
    }
}

impl Editor {
    fn show(&mut self, ctx: &egui::Context) {
        if let Some(gesture) = &mut self.gesture_preview
            && let Some(pixels) = gesture.preview.poll()
        {
            let result = if let Effect::Move(parameters) = gesture.effect {
                self.document
                    .publish_move_preview(pixels, parameters.dx, parameters.dy)
            } else {
                self.document.publish_preview(pixels)
            };
            match result {
                Ok(()) => self.texture_dirty = true,
                Err(error) => self.status = error,
            }
        }
        if self.gesture_preview.is_some() && ctx.input(|i| i.viewport().close_requested()) {
            self.shape_edit = None;
            self.curve_controls = None;
            self.gesture_preview = None;
            self.document.cancel();
            self.last_point = None;
            self.shape_origin = None;
            self.texture_dirty = true;
        }
        self.document.defer_history_storage(true);
        self.poll_file_job(ctx);
        self.open_queue.extend(ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect::<Vec<_>>()
        }));
        if self.file_job.is_none()
            && self.pending.is_none()
            && self.settings_dialog.is_none()
            && !self.new_dialog
            && self.export_dialog.is_none()
            && self.text_dialog.is_none()
            && self.effect_dialog.is_none()
            && self.selection_transform.is_none()
            && self.paste_confirmation.is_none()
            && self.resize_dialog.is_none()
            && self.layer_properties.is_none()
            && !self.document.editing()
            && self.last_point.is_none()
            && let Some(path) = self.open_queue.pop_front()
        {
            self.load(path);
        }
        self.document.defer_history_storage(true);
        if let Some(error) = self.document.history_error() {
            self.status = error.to_owned();
        }
        if self.file_job.is_some() && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            self.cancel_selection_transform();
            self.paste_confirmation = None;
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.effect_dialog.is_some() {
            self.effect_dialog = None;
            self.document.cancel();
            self.texture_dirty = true;
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.export_dialog.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.status = "Finish or cancel the export before closing".into();
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.text_dialog.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.status = "Apply or cancel the text before closing".into();
        }
        if let Some(receiver) = &self.appearance
            && let Some(theme) = receiver.try_iter().last()
        {
            self.system_theme = Some(theme);
            if self.preferences.theme == preferences::Theme::System {
                self.apply_theme(ctx);
            }
        }
        if !ctx.input(|i| i.focused) {
            self.finish_stroke();
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && self.file_job.is_none()
            && self.export_dialog.is_none()
            && self.text_dialog.is_none()
            && (self.document.dirty() || !self.documents.is_empty())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.pending.is_none() {
                self.request(Action::Close, ctx);
            }
        }
        let enabled = self.file_job.is_none()
            && self.pending.is_none()
            && self.settings_dialog.is_none()
            && !self.new_dialog
            && self.layer_properties.is_none()
            && self.resize_dialog.is_none()
            && self.export_dialog.is_none()
            && self.text_dialog.is_none()
            && self.effect_dialog.is_none()
            && self.selection_transform.is_none()
            && self.paste_confirmation.is_none()
            && !self.recovery_prompt;

        if enabled {
            self.shortcuts(ctx);
        }
        let enabled = self.file_job.is_none()
            && self.pending.is_none()
            && self.settings_dialog.is_none()
            && !self.new_dialog
            && self.layer_properties.is_none()
            && self.resize_dialog.is_none()
            && self.export_dialog.is_none()
            && self.text_dialog.is_none()
            && self.effect_dialog.is_none()
            && self.selection_transform.is_none()
            && self.paste_confirmation.is_none()
            && !self.recovery_prompt;
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into());
        let title = format!(
            "{}{} — Paint Rust",
            name,
            if self.document.dirty() { " *" } else { "" }
        );
        ctx.accesskit_node_builder(egui::accesskit_root_id(), |node| {
            node.set_label(title.clone())
        });
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            if !enabled || self.gesture_preview.is_some() {
                ui.disable();
            }
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New    Ctrl+N").clicked() {
                        self.request(Action::New, ctx);
                        ui.close();
                    }
                    if ui.button("Open…    Ctrl+O").clicked() {
                        self.request(Action::Open, ctx);
                        ui.close();
                    }
                    self.recent_menu(ui);
                    if !self.recoverable.is_empty() && ui.button("Recover…").clicked() {
                        self.request(Action::Recovery, ctx);
                    }
                    ui.menu_button("Export", |ui| {
                        for format in files::RasterFormat::ALL {
                            if ui.add(Icon::SaveAs.button(format.label())).clicked() {
                                self.export_image(format);
                                ui.close();
                            }
                        }
                    });

                    ui.separator();
                    if ui.button("Save    Ctrl+S").clicked() {
                        self.save(false);
                        ui.close();
                    }
                    if ui.button("Save as…    Ctrl+Shift+S").clicked() {
                        self.save(true);
                        ui.close();
                    }
                    if ui.button("Save all    Ctrl+Alt+S").clicked() {
                        self.request(Action::SaveAll, ctx);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Close document    Ctrl+W").clicked() {
                        self.request(Action::CloseDocument, ctx);
                        ui.close();
                    }
                    if ui.button("Close all").clicked() {
                        self.request(Action::CloseAll, ctx);
                        ui.close();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui
                        .add_enabled(
                            self.document.can_undo(),
                            Icon::Undo.button("Undo    Ctrl+Z"),
                        )
                        .clicked()
                    {
                        self.undo();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.document.can_redo(),
                            Icon::Redo.button("Redo    Ctrl+Y"),
                        )
                        .clicked()
                    {
                        self.redo();
                        ui.close();
                    }
                    ui.separator();
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
                        .add(Icon::Paste.button("Paste    Ctrl+V"))
                        .clicked()
                    {
                        self.paste();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Paste into new layer    Ctrl+Shift+V").clicked() {self.paste_new_layer();ui.close();}
                    if ui.button("Copy selection geometry    Ctrl+Alt+Shift+C").clicked() {self.copy_selection_geometry();ui.close();}
                    ui.menu_button("Paste selection geometry",|ui| {
                        for mode in [CombineMode::Replace,CombineMode::Add,CombineMode::Subtract,CombineMode::Intersect,CombineMode::Xor] {
                            if ui.button(mode.label()).clicked() {self.paste_selection_geometry(mode);ui.close();}
                        }
                    });
                    if ui.button("Paste into new image    Ctrl+Alt+V").clicked() {self.paste_new_image();ui.close();}
                    if ui.button("Fill selection with primary    Backspace").clicked() {self.start_document_edit(DocumentEdit::Fill(self.color.to_srgba_unmultiplied()));ui.close();}
                    if ui.button("Fill selection with secondary    Shift+Backspace").clicked() {self.start_document_edit(DocumentEdit::Fill(self.secondary_color.to_srgba_unmultiplied()));ui.close();}
                });
                ui.menu_button("Image", |ui| {
                    for (label, canvas) in [("Resize image…    Ctrl+R", false), ("Canvas size…    Ctrl+Shift+R", true)] {
                        if ui.button(label).clicked() {
                            self.open_resize_dialog(canvas);
                            ui.close();
                        }
                    }
                    ui.separator();
                    for (label, operation) in [
                        ("Rotate 90° clockwise    Ctrl+H", Orientation::Clockwise),
                        ("Rotate 90° counterclockwise    Ctrl+G", Orientation::Counterclockwise),
                        ("Rotate 180°", Orientation::HalfTurn),
                        ("Flip horizontal", Orientation::FlipHorizontal),
                        ("Flip vertical", Orientation::FlipVertical),
                    ] {
                        if ui.button(label).clicked() {
                            self.finish_stroke();
                            self.transform_in_background(
                                "transform image",
                                move |document, cancel| {
                                    document.reorient_cancellable(operation, cancel)
                                },
                            );
                            ui.close();
                        }
                    }
                });
                ui.menu_button("Layers", |ui| {
                    self.layer_menu_contents(ui);
                });
                ui.menu_button("Selection", |ui| {
                    if ui.add_enabled(self.document.selection().is_some(),egui::Button::new("Transform selection boundary…    Ctrl+Alt+M")).clicked() {
                        self.begin_selection_transform(ctx);ui.close();
                    }

                    if ui.button("Free transform pixels…").clicked() {
                        self.begin_effect(ctx, Effect::FreeTransform(Default::default()));
                        ui.close();
                    }
                    if ui.button("Select all    Ctrl+A").clicked() {
                        self.finish_stroke();
                        self.document.set_selection_with_history(Some(Selection {mask:None,ellipse:None,x:0,y:0,width:self.document.width,height:self.document.height}),"Select all");
                        ui.close();
                    }
                    if ui.button("Invert selection    Ctrl+I").clicked() {
                        self.invert_selection();
                        ui.close();
                    }
                    if ui.button("Deselect    Ctrl+D").clicked() {
                        self.finish_stroke();self.document.set_selection_with_history(None,"Deselect");
                        ui.close();
                    }
                    if ui.button("Delete pixels    Del").clicked() {
                        self.finish_stroke();
                        self.start_document_edit(DocumentEdit::Delete);
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
                        self.transform_in_background(
                            "crop image",
                            Document::crop_to_selection_cancellable,
                        );
                        ui.close();
                    }
                });
                let effect_groups: [(&str, &[Effect]); 2] = [
                    (
                        "Adjustments",
                        &[
                            Effect::InvertAlpha,
                            Effect::TemperatureTint {
                                temperature: 0,
                                tint: 0,
                            },
                            Effect::HighlightsShadows {
                                highlights: 0,
                                shadows: 0,
                            },
                            Effect::Exposure {
                                stops: 0.0,
                                offset: 0.0,
                                gamma: 1.0,
                            },
                            Effect::Curves(Default::default()),
                            Effect::Levels(Default::default()),
                            Effect::AutoLevels(0.5),
                            Effect::BrightnessContrast {
                                brightness: 0,
                                contrast: 0,
                            },
                            Effect::HueSaturation {
                                hue: 0,
                                saturation: 1.0,
                                lightness: 0,
                            },
                            Effect::Grayscale,
                            Effect::Sepia,
                            Effect::Posterize(4),
                        ],
                    ),
                    (
                        "Effects",
                        &[
                            Effect::Bulge(30.0),
                            Effect::Twist(45.0),
                            Effect::FrostedGlass { radius: 3, seed: 1 },
                            Effect::Crystalize { size: 12, seed: 1 },
                            Effect::Vignette {
                                amount: 0.5,
                                roundness: 0.5,
                            },
                            Effect::RedEye(1.5),
                            Effect::AddNoise {
                                amount: 20,
                                coverage: 100,
                                monochrome: true,
                                seed: 1,
                            },
                            Effect::Median {
                                radius: 1,
                                percentile: 50,
                            },
                            Effect::EdgeDetect,
                            Effect::Emboss(45.0),
                            Effect::RadialBlur {
                                angle: 10.0,
                                center: [0.5, 0.5],
                            },
                            Effect::ZoomBlur {
                                amount: 0.1,
                                center: [0.5, 0.5],
                            },
                            Effect::FragmentBlur {
                                distance: 5.0,
                                fragments: 4,
                                angle: 0.0,
                            },
                            Effect::SurfaceBlur {
                                radius: 3,
                                threshold: 30,
                            },
                            Effect::DropShadow {
                                radius: 4,
                                offset: [4, 4],
                                color: [0, 0, 0, 128],
                            },
                            Effect::Glow {
                                radius: 6,
                                amount: 0.5,
                            },
                            Effect::SoftenPortrait {
                                radius: 4,
                                softness: 0.5,
                                warmth: 0.1,
                            },
                            Effect::SketchBlur {radius:5,percentile:50,smoothness:2,seed:1},
                            Effect::Straighten {angle:0.0,auto_crop:true},
                            Effect::BokehBlur {radius:4.0,gamma:1.0,quality:2},
                            Effect::Quantize(16),
                            Effect::OilPainting {radius:3,bins:16},
                            Effect::PencilSketch {radius:4,strength:1.0},
                            Effect::InkSketch {threshold:40,color:1.0},
                            Effect::ReduceNoise {radius:1,strength:0.75},
                            Effect::Relief {angle:45.0,strength:1.0},
                            Effect::Outline {radius:1,intensity:1.0},
                            Effect::Morphology {radius:1,operation:paint_rust::effects::MorphologyOperation::Dilate},
                            Effect::Dents {scale:40.0,amount:10.0,seed:1},
                            Effect::PolarInversion {amount:1.0,center:[0.5,0.5]},
                            Effect::TileReflection {size:40.0,amount:8.0,angle:0.0},
                            Effect::Clouds(paint_rust::effects::NoiseSettings::default()),
                            Effect::Turbulence(paint_rust::effects::NoiseSettings::default()),
                            Effect::Mandelbrot(paint_rust::effects::FractalSettings::default()),
                            Effect::Julia(paint_rust::effects::FractalSettings { center:[0.0,0.0], ..Default::default() }),
                            Effect::GaussianBlur(3),
                            Effect::MotionBlur {
                                distance: 10,
                                angle: 0.0,
                                centered: true,
                            },
                            Effect::BoxBlur(2),
                            Effect::Sharpen(1.0),
                            Effect::Pixelate(8),
                        ],
                    ),
                ];
                for (title, effects) in effect_groups {
                    ui.menu_button(title, |ui| {
                        if title == "Adjustments"
                            && ui
                                .add(Icon::Invert.button("Invert colors    Ctrl+Shift+I"))
                                .clicked()
                        {
                            self.start_document_edit(DocumentEdit::Invert);
                            ui.close();
                        }
                        if title == "Effects" {
                            let name = self.last_effect.map(Effect::name).unwrap_or("last effect");
                            if ui
                                .add_enabled(
                                    self.last_effect.is_some(),
                                    egui::Button::new(format!("Repeat {name}    Ctrl+F")),
                                )
                                .clicked()
                            {
                                self.repeat_effect();
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    self.last_effect.is_some(),
                                    egui::Button::new("Last effect settings…    Ctrl+Alt+F"),
                                )
                                .clicked()
                            {
                                if let Some(effect) = self.last_effect {
                                    self.begin_effect(ctx, effect);
                                }
                                ui.close();
                            }
                            ui.separator();
                        }
                        if title == "Effects" {
                            for category in
                                ["Artistic", "Blurs", "Color", "Distort", "Noise", "Object", "Photo", "Render", "Stylize"]
                            {
                                ui.menu_button(category, |ui| {
                                    for &effect in effects
                                        .iter()
                                        .filter(|effect| effect.category() == category)
                                    {
                                        if ui.button(effect.name()).clicked() {
                                            self.begin_effect(ctx, effect);
                                            ui.close();
                                        }
                                    }
                                });
                            }
                        } else {
                            for &effect in effects {
                                if ui.button(effect.name()).clicked() {
                                    self.begin_effect(ctx, effect);
                                    ui.close();
                                }
                            }
                        }
                    });
                }
                ui.menu_button("View", |ui| {
                    if ui.button("Preferences…    Alt+X").clicked() { self.begin_settings(); ui.close(); }
                    for (label, direction) in [("Zoom in    Ctrl++", true), ("Zoom out    Ctrl+-", false)] {
                        if ui.button(label).clicked() { self.step_view_zoom(direction); ui.close(); }
                    }
                    if ui.button("Actual size    Ctrl+0").clicked() { self.set_view_zoom(1.0); ui.close(); }
                    self.preferences_menu(ui);
                    ui.checkbox(&mut self.preferences.rulers, "Rulers");
                    ui.menu_button("Measurement units",|ui| { for unit in paint_rust::resolution::Unit::ALL {ui.selectable_value(&mut self.preferences.units,unit,unit.label());} });
                    if ui.button("Fit image    Ctrl+B").clicked() {
                        self.fit_canvas = true;
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.document.selection().is_some(),
                            egui::Button::new("Fit selection    Ctrl+Shift+B"),
                        )
                        .clicked()
                    {
                        self.zoom_to_selection();
                        ui.close();
                    }
                    ui.checkbox(&mut self.panel_layout.floating, "Floating panels").on_hover_text("Small windows temporarily use scrollable side panels; enlarging restores floating positions.");
                    ui.checkbox(&mut self.pixel_grid, "Pixel grid (at 800% and above)");
                    for (index, name) in panels::NAMES.iter().enumerate() {
                        ui.checkbox(
                            &mut self.panel_layout.visible[index],
                            format!("{name}    F{}", index + 5),
                        );
                    }
                    if ui.button("Reset panel layout").clicked() {
                        self.panel_layout = Default::default();
                        self.panel_epoch += 1;
                        ui.close();
                    }
                });
                ui.menu_button("Window",|ui| self.document_menu(ui));
                ui.menu_button("Help",|ui| {
                    if ui.button("Guide and shortcuts    F1").clicked() { self.help_open=true; ui.close(); }
                    if ui.button("Operation errors…    Ctrl+F1").clicked() { self.errors.open=true; ui.close(); }
                    ui.separator();
                    if ui.button("About Paint Rust…").clicked() { self.about.open=true; ui.close(); }
                });
            });
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
                ui.separator();
                if ui
                    .add(Icon::Cut.button("Cut"))
                    .on_hover_text("Ctrl+X")
                    .clicked()
                {
                    self.copy(true, false);
                }
                if ui
                    .add(Icon::Copy.button("Copy"))
                    .on_hover_text("Ctrl+C")
                    .clicked()
                {
                    self.copy(false, false);
                }
                if ui
                    .add(Icon::Paste.button("Paste"))
                    .on_hover_text("Ctrl+V")
                    .clicked()
                {
                    self.paste();
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.strong("Tool options");
                self.tool_options(ui);
                ui.separator();
                toolbar::slider(
                    ui,
                    egui::Slider::new(&mut self.zoom, 0.01..=64.0)
                        .logarithmic(true)
                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                    "Zoom",
                );
                if ui.add(Icon::ActualSize.button("100%")).clicked() {
                    self.set_view_zoom(1.0);
                }
                if ui.add(Icon::Fit.button("Fit")).clicked() {
                    self.fit_canvas = true;
                }
            });
        });
        self.document_tabs(
            ctx,
            enabled && self.file_job.is_none() && self.shape_edit.is_none(),
        );
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} × {} px  |  RGBA  |  {:.0}%",
                    self.document.width,
                    self.document.height,
                    self.zoom * 100.0
                ));
                if self.canvas_renderer.pending {
                    ui.spinner().on_hover_text("Updating canvas preview");
                }
                ui.separator();
                if self.errors.has_entries() && ui.button("Errors…").clicked() {
                    self.errors.open = true;
                }
                ui.add(egui::Label::new(&self.status).truncate())
                    .on_hover_text(&self.status);
            });
        });
        self.workspace_panels(ctx, enabled && self.shape_edit.is_none());
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(&ctx.style())
                    .fill(crate::appearance::workspace_color(ctx.theme())),
            )
            .show(ctx, |ui| {
                if !enabled || self.file_job.is_some() {
                    ui.disable();
                }
                self.canvas(ui);
            });
        self.dialogs(ctx);
        self.show_settings(ctx);
        self.show_shape_edit(ctx);
        self.show_paste_confirmation(ctx);
        self.show_selection_transform(ctx);
        self.show_help(ctx);
        self.errors.show(ctx);
        self.about.show(ctx);
        if let Some(recovery) = &mut self.recovery {
            let _ = recovery.set_context(paint_rust::recovery::Context {
                version: 1,
                workspace: recovery.workspace().to_owned(),
                order: self
                    .document_order
                    .iter()
                    .position(|id| *id == self.active_document_id)
                    .unwrap_or(0),
                path: self.path.as_ref().and_then(|p| std::path::absolute(p).ok()),
                zoom: self.zoom,
                offset: [self.canvas_offset.x, self.canvas_offset.y],
                active: true,
            });
            let wake = ctx.clone();
            if let Err(error) = recovery.tick(&self.document, move || wake.request_repaint()) {
                self.status = format!("Could not update the recovery copy: {error}");
            }
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        self.tick_inactive_recovery(ctx);
        if !ctx.input(|i| i.pointer.button_down(self.stroke_button)) {
            self.finish_stroke();
            self.suppress_stroke = false;
        }
        if enabled
            && !self.allow_close
            && !self.document.editing()
            && self.selection_transform.is_none()
            && self.paste_confirmation.is_none()
            && self.workspace_history_needs_storage(workspace::HISTORY_RAM_BUDGET)
        {
            self.store_workspace_history_async(workspace::HISTORY_RAM_BUDGET, false);
        }
        if self.file_job.is_some()
            && !self.allow_close
            && ctx.input(|i| i.viewport().close_requested())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if self.texture_dirty {
            ctx.request_repaint();
        }
    }
}

impl eframe::App for Editor {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Err(error) = effect_settings::Settings::new(
            self.last_effect,
            self.effect_parameters.values().copied().collect(),
        )
        .save()
        {
            eprintln!("Could not save effect settings: {error}");
        }
        if let Err(error) = self.current_general_settings().save() {
            eprintln!("Could not save general preferences: {error}");
        }
        self.preferences.brush_size = self.brush_size;
        self.preferences.brush = self.brush_options;
        self.preferences.pixel_grid = self.pixel_grid;
        if let Err(error) = self.preferences.save() {
            eprintln!("Could not save preferences: {error}");
        }
        self.clear_recovery();
        self.clear_inactive_recovery();
        if let Err(error) = self.color_panel.save() {
            eprintln!("Could not save palette: {error}");
        }
        if let Err(error) = self.panel_layout.save() {
            eprintln!("Could not save workspace layout: {error}");
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show(ctx);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn percentage_resize_uses_original_dimensions_and_preserves_ratio_at_limits() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(100, 50, [255; 4]).unwrap();
        editor.open_resize_dialog(false);
        let mut dialog = editor.resize_dialog.take().unwrap();
        for (percent, expected) in [
            (50.0, (50, 25)),
            (200.0, (200, 100)),
            (1e6, (16384, 8192)),
            (0.01, (1, 1)),
        ] {
            dialog.percent = percent;
            dialog.apply_percentage();
            assert_eq!((dialog.width, dialog.height), expected);
        }
    }

    #[test]
    fn resolution_only_dialog_apply_preserves_pixels_and_has_one_undo_step() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(300, 150, [40, 80, 120, 255]).unwrap();
        let selected = Some(Selection::elliptical(10, 10, 30, 30));
        editor.document.set_selection(selected.clone());
        editor.open_resize_dialog(false);
        let mut dialog = editor.resize_dialog.take().unwrap();
        dialog.dpi = paint_rust::resolution::Dpi { x: 300.0, y: 300.0 };
        dialog.units = paint_rust::resolution::Unit::Inches;
        assert!(editor.apply_resize_dialog(&dialog));
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.resolution(), dialog.dpi);
        assert_eq!(editor.document.selection(), selected);
        assert_eq!((editor.document.width, editor.document.height), (300, 150));
        assert!(
            editor
                .document
                .pixels()
                .iter()
                .all(|p| *p == [40, 80, 120, 255])
        );
        assert_eq!(editor.document.history().count(), 1);
        editor.undo();
        assert_eq!(editor.document.resolution(), Default::default());
        assert!(!editor.document.dirty());
        dialog.width = 600;
        dialog.height = 300;
        assert!(editor.apply_resize_dialog(&dialog));
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.history().count(), 1);
        assert_eq!(editor.document.resolution(), dialog.dpi);
        editor.undo();
        assert_eq!((editor.document.width, editor.document.height), (300, 150));
        assert_eq!(editor.document.resolution(), Default::default());
    }
    #[test]
    fn image_and_layer_shortcuts_route_to_their_commands_without_collisions() {
        use egui::Modifiers as M;
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(3, 2, [40, 50, 60, 255]).unwrap();
        let press = |editor: &mut Editor, key, modifiers| {
            frame(
                editor,
                &ctx,
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
            );
            wait_for_files(editor, &ctx);
        };
        press(&mut editor, Key::N, M::COMMAND | M::SHIFT);
        assert_eq!(editor.document.layers().len(), 2);
        assert!(!editor.new_dialog);
        press(&mut editor, Key::D, M::COMMAND | M::SHIFT);
        assert_eq!(editor.document.layers().len(), 3);
        let history = editor.document.history().count();
        for (key, modifiers, index) in [
            (Key::PageDown, M::ALT, 1),
            (Key::PageUp, M::ALT, 2),
            (Key::PageDown, M::ALT | M::COMMAND, 0),
            (Key::PageUp, M::ALT | M::COMMAND, 2),
        ] {
            press(&mut editor, key, modifiers);
            assert_eq!(editor.document.active_layer(), index);
        }
        assert_eq!(editor.document.history().count(), history);
        press(&mut editor, Key::Comma, M::COMMAND);
        assert!(!editor.document.layers()[2].visible);
        press(&mut editor, Key::Delete, M::COMMAND | M::SHIFT);
        assert_eq!(editor.document.layers().len(), 2);
        press(&mut editor, Key::H, M::COMMAND);
        assert_eq!((editor.document.width, editor.document.height), (2, 3));
        press(&mut editor, Key::G, M::COMMAND);
        assert_eq!((editor.document.width, editor.document.height), (3, 2));
        editor.last_effect = Some(Effect::BrightnessContrast {
            brightness: 10,
            contrast: 0,
        });
        press(&mut editor, Key::F, M::COMMAND | M::SHIFT);
        assert_eq!(editor.document.layers().len(), 1);
        assert!(editor.effect_dialog.is_none());
        press(&mut editor, Key::F, M::COMMAND | M::ALT);
        assert!(editor.effect_dialog.is_some());
        press(&mut editor, Key::Escape, M::NONE);
        for (modifiers, canvas) in [(M::COMMAND, false), (M::COMMAND | M::SHIFT, true)] {
            press(&mut editor, Key::R, modifiers);
            assert_eq!(editor.resize_dialog.as_ref().unwrap().canvas, canvas);
            editor.resize_dialog = None;
        }
    }
    #[test]
    fn selection_boundary_handles_move_preview_and_commit_clean_undo() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        let selection = Selection {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
            mask: None,
            ellipse: None,
        };
        editor.document.set_selection(Some(selection.clone()));
        let pixels = editor.document.shared_pixels();
        editor.begin_selection_transform(&ctx);
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![]);
        let point = editor.canvas_viewport.min + Vec2::new(60.0, 500.0);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(point), pointer(point, true)],
        );
        assert!(editor.transform_drag.is_some());
        let end = point + Vec2::new(30.0, 20.0);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(end), pointer(end, false)],
        );
        assert!(editor.transform_drag.is_none());
        let (_, parameters, _) = editor.selection_transform.as_ref().unwrap().handle_state();
        assert_eq!((parameters.offset_x, parameters.offset_y), (30.0, 20.0));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while editor.document.selection().unwrap().x != 30 {
            frame(&mut editor, &ctx, vec![]);
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(!editor.document.can_undo());
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(editor.selection_transform.is_none());
        assert_eq!(editor.document.pixels(), pixels.as_slice());
        assert!(!editor.document.dirty());
        assert_eq!(editor.document.history().count(), 1);
        editor.undo();
        assert_eq!(editor.document.selection(), Some(selection));
    }
    #[test]
    fn recover_all_keeps_existing_work_and_skips_corrupt_snapshots() {
        use paint_rust::recovery::Recovery;
        let ctx = egui::Context::default();
        let root = tempfile::tempdir().unwrap();
        for color in [[10, 20, 30, 255], [40, 50, 60, 128], [70, 80, 90, 0]] {
            let mut recovery = Recovery::new(root.path()).unwrap();
            let mut document = Document::new(3, 2, color).unwrap();
            document.mark_unsaved();
            recovery.schedule(&document, || {}).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while recovery.busy() {
                recovery.poll().unwrap();
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(2, 2, [255; 4]).unwrap();
        editor.document.mark_unsaved();
        let existing = editor.active_document_id;
        editor.recovery = Some(Recovery::new(root.path()).unwrap());
        editor.recoverable = Recovery::discover(root.path()).unwrap();
        assert_eq!(editor.recoverable.len(), 3);
        let broken = editor.recoverable[1].path.clone();
        std::fs::write(&broken, b"invalid recovery snapshot").unwrap();
        editor.start_recovery_batch();
        wait_for_files(&mut editor, &ctx);
        assert!(editor.recovery_batch.is_none());
        assert_eq!(editor.documents.len(), 2);
        assert_eq!(editor.recoverable.len(), 1);
        assert_eq!(editor.recoverable[0].path, broken);
        assert!(broken.exists());
        assert!(editor.recovery_prompt);
        assert!(editor.status.contains("Recovered 2 document(s); 1 failed"));
        assert!(editor.document.dirty());
        assert!(editor.path.is_none());
        editor.activate_document(existing);
        assert_eq!(editor.active_document_id, existing);
        assert_eq!(editor.document.pixels(), &[[255; 4]; 4]);
        for _ in 0..3 {
            assert!(editor.document.dirty());
            editor.cycle_document(1);
        }
    }
    #[test]
    fn recovered_context_restores_paths_views_order_and_active_tab() {
        use paint_rust::recovery::{Context, Recovery};
        let ctx = egui::Context::default();
        let root = tempfile::tempdir().unwrap();
        let mut sessions = vec![Recovery::new(root.path()).unwrap()];
        sessions.push(sessions[0].sibling().unwrap());
        for (i, recovery) in sessions.iter_mut().enumerate() {
            let mut document = Document::new(400, 300, [20 + i as u8, 40, 60, 255]).unwrap();
            document.mark_unsaved();
            recovery
                .set_context(Context {
                    version: 1,
                    workspace: recovery.workspace().into(),
                    order: 1 - i,
                    path: Some(root.path().join(format!("drawing-{i}.pdn"))),
                    zoom: 2.0 + i as f32,
                    offset: [20.0, 30.0],
                    active: i == 1,
                })
                .unwrap();
            recovery.schedule(&document, || {}).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while recovery.busy() {
                recovery.poll().unwrap();
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
        drop(sessions);
        let mut editor = Editor::blank(&ctx);
        editor.recovery = Some(Recovery::new(root.path()).unwrap());
        editor.recoverable = Recovery::discover(root.path()).unwrap();
        editor.start_recovery_batch();
        wait_for_files(&mut editor, &ctx);
        assert!(editor.recovery_batch.is_none());
        assert_eq!(editor.path, Some(root.path().join("drawing-1.pdn")));
        assert_eq!(editor.zoom, 3.0);
        assert_eq!(editor.canvas_offset, Vec2::new(20.0, 30.0));
        assert_eq!(editor.document_order[0], editor.active_document_id);
        assert!(editor.document.dirty());
        let last = editor.document_order[1];
        editor.activate_document(last);
        assert_eq!(editor.path, Some(root.path().join("drawing-0.pdn")));
        assert_eq!(editor.zoom, 2.0);
        assert!(editor.document.dirty());
    }
    #[test]
    fn zoom_commands_preserve_view_center_and_never_edit_document() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.canvas_viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0));
        editor.canvas_offset = Vec2::new(150.0, 150.0);
        let center = (editor.canvas_offset + Vec2::splat(50.0)) / editor.zoom;
        editor.step_view_zoom(true);
        assert_eq!(editor.zoom, 1.5);
        assert_eq!(
            (editor.canvas_offset + Vec2::splat(50.0)) / editor.zoom,
            center
        );
        editor.step_view_zoom(false);
        assert_eq!(editor.zoom, 1.0);
        editor.set_view_zoom(0.01);
        editor.step_view_zoom(false);
        assert_eq!(editor.zoom, 0.01);
        editor.set_view_zoom(64.0);
        editor.step_view_zoom(true);
        assert_eq!(editor.zoom, 64.0);
        for (key, expected) in [
            (Key::Num0, 1.0),
            (Key::Plus, 1.5),
            (Key::Minus, 1.0),
            (Key::Equals, 1.5),
        ] {
            frame(
                &mut editor,
                &ctx,
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                }],
            );
            assert_eq!(editor.zoom, expected);
        }
        assert!(!editor.document.dirty());
        assert!(!editor.document.can_undo());
    }
    #[test]
    fn erase_and_cut_restore_selection_in_one_undo_even_without_pixel_changes() {
        let ctx = egui::Context::default();
        for cut in [false, true] {
            for already_clear in [false, true] {
                let mut editor = Editor::blank(&ctx);
                let cleared = if cut { [255, 255, 255, 0] } else { [0; 4] };
                let initial = if already_clear {
                    cleared
                } else {
                    [10, 20, 30, 255]
                };
                editor.document = Document::new(5, 5, initial).unwrap();
                let selected = Some(Selection::elliptical(1, 1, 3, 3));
                editor.document.set_selection(selected.clone());
                *editor.clipboard.lock().unwrap() = Some(Box::new(MemoryClipboard {
                    image: None,
                    fail: false,
                }));
                if cut {
                    editor.copy(true, false);
                } else {
                    editor.start_document_edit(DocumentEdit::Delete);
                }
                wait_for_files(&mut editor, &ctx);
                assert!(editor.document.selection().is_none());
                assert_eq!(editor.document.pixels()[0], initial);
                assert_eq!(editor.document.pixels()[12], cleared);
                assert_eq!(editor.document.dirty(), !already_clear);
                assert_eq!(editor.document.history().count(), 1);
                editor.undo();
                assert_eq!(editor.document.selection(), selected);
                assert_eq!(editor.document.pixels(), &vec![initial; 25]);
                assert!(!editor.document.dirty());
                editor.redo();
                assert!(editor.document.selection().is_none());
                assert_eq!(editor.document.pixels()[12], cleared);
            }
        }
    }
    #[test]
    fn oversized_paste_waits_for_choice_and_expands_in_one_undo_step() {
        let ctx = egui::Context::default();
        for new_layer in [false, true] {
            let mut editor = Editor::blank(&ctx);
            editor.document = Document::new(3, 2, [10, 20, 30, 255]).unwrap();
            let original = editor.document.pixels().to_vec();
            let selection = Some(Selection {
                x: 2,
                y: 1,
                width: 1,
                height: 1,
                mask: None,
                ellipse: None,
            });
            editor.document.set_selection(selection.clone());
            editor.receive_paste_image(paste::Image {
                width: 2,
                height: 2,
                pixels: vec![[100, 50, 25, 128]; 4],
                new_layer,
            });
            assert!(editor.paste_confirmation.is_some());
            assert!(editor.file_job.is_none());
            assert_eq!(editor.document.pixels(), original);
            assert!(!editor.document.dirty());
            let image = editor.paste_confirmation.take().unwrap();
            editor.apply_paste_image(image, true);
            wait_for_files(&mut editor, &ctx);
            assert_eq!((editor.document.width, editor.document.height), (4, 3));
            assert_eq!(
                editor.document.layers().len(),
                if new_layer { 2 } else { 1 }
            );
            assert_eq!(editor.document.pixels()[6], [100, 50, 25, 128]);
            assert_eq!(editor.document.pixels()[11], [100, 50, 25, 128]);
            assert_eq!(editor.document.history().count(), 1);
            assert!(editor.document.undo());
            assert_eq!((editor.document.width, editor.document.height), (3, 2));
            assert_eq!(editor.document.pixels(), original);
            assert_eq!(editor.document.selection(), selection);
            assert!(!editor.document.dirty());
        }
    }
    #[test]
    fn oversized_paste_can_clip_or_cancel_without_expanding() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(2, 2, [255; 4]).unwrap();
        editor.receive_paste_image(paste::Image {
            width: 3,
            height: 3,
            pixels: vec![[12, 34, 56, 78]; 9],
            new_layer: false,
        });
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
        assert!(editor.paste_confirmation.is_none());
        assert!(!editor.document.dirty());
        assert!(!editor.document.can_undo());
        editor.apply_paste_image(
            paste::Image {
                width: 3,
                height: 3,
                pixels: vec![[12, 34, 56, 78]; 9],
                new_layer: false,
            },
            false,
        );
        wait_for_files(&mut editor, &ctx);
        assert_eq!((editor.document.width, editor.document.height), (2, 2));
        assert_eq!(editor.document.pixels(), &[[12, 34, 56, 78]; 4]);
        assert!(editor.status.contains("clipped"));
        assert!(editor.document.undo());
        assert_eq!(editor.document.pixels(), &[[255; 4]; 4]);
    }
    #[test]
    fn clone_requires_source_and_recolor_uses_secondary_color() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.tool = Tool::Clone;
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![]);
        let point = editor.canvas_viewport.min + Vec2::new(100.0, 100.0);
        for pressed in [true, false] {
            frame(
                &mut editor,
                &ctx,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        pressed,
                        button: egui::PointerButton::Primary,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(!editor.document.dirty());
        assert!(!editor.document.editing());
        editor.tool = Tool::Recolor;
        editor.color = Color32::RED.into();
        editor.secondary_color = Color32::WHITE.into();
        for pressed in [true, false] {
            frame(
                &mut editor,
                &ctx,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        pressed,
                        button: egui::PointerButton::Primary,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(editor.document.pixels().contains(&[255, 0, 0, 255]));
        assert!(editor.document.undo());
        assert!(editor.document.pixels().iter().all(|p| *p == [255; 4]));
    }

    #[test]
    fn recolor_samples_once_per_stroke_and_uses_each_buttons_target() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        let a = [20, 40, 60, 255];
        let b = [80, 100, 120, 255];
        editor.document = Document::from_pixels(
            200,
            200,
            (0..40000)
                .map(|i| if i % 200 < 50 { a } else { b })
                .collect(),
        )
        .unwrap();
        editor.tool = Tool::Recolor;
        editor.recolor_once = true;
        editor.brush_size = 8.0;
        editor.color = Color32::RED.into();
        editor.secondary_color = Color32::GREEN.into();
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![]);
        let start = editor.canvas_viewport.min + Vec2::new(20.5, 20.5);
        let end = start + Vec2::new(60.0, 0.0);
        let event = |pos, pressed, button| egui::Event::PointerButton {
            pos,
            pressed,
            button,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &mut editor,
            &ctx,
            vec![
                egui::Event::PointerMoved(start),
                event(start, true, egui::PointerButton::Primary),
            ],
        );
        assert_eq!(editor.recolor_sample, Some(a));
        frame(&mut editor, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(editor.recolor_sample, Some(a));
        frame(
            &mut editor,
            &ctx,
            vec![event(end, false, egui::PointerButton::Primary)],
        );
        assert!(editor.document.pixels().contains(&[255, 0, 0, 255]));
        assert_eq!(
            editor.document.pixels().iter().filter(|p| **p == b).count(),
            150 * 200
        );
        assert!(editor.recolor_sample.is_none());
        frame(
            &mut editor,
            &ctx,
            vec![
                egui::Event::PointerMoved(end),
                event(end, true, egui::PointerButton::Secondary),
            ],
        );
        assert_eq!(editor.recolor_sample, Some(b));
        frame(
            &mut editor,
            &ctx,
            vec![event(end, false, egui::PointerButton::Secondary)],
        );
        assert!(editor.document.pixels().contains(&[0, 255, 0, 255]));
        assert_eq!(editor.document.history().count(), 2);
        editor.undo();
        editor.undo();
        assert_eq!(
            editor.document.pixels().iter().filter(|p| **p == a).count(),
            50 * 200
        );
        assert!(!editor.document.dirty());
    }
    #[test]
    fn raster_save_matches_decoded_file_and_keeps_layered_undo() {
        let ctx = egui::Context::default();
        let directory = tempfile::tempdir().unwrap();
        for format in files::RasterFormat::ALL {
            let mut editor = Editor::blank(&ctx);
            editor.document = Document::new(4, 3, [100, 150, 200, 128]).unwrap();
            editor.document.add_layer(true);
            editor
                .document
                .set_layer_style("Upper".into(), 180, BlendMode::Screen);
            let before = editor.document.layers().to_vec();
            let path = directory
                .path()
                .join(format!("saved.{}", format.extension()));
            let bytes = files::encode_raster_bytes(
                &editor.document.composite(),
                (4, 3),
                files::RasterOptions::new(format),
                &std::sync::atomic::AtomicBool::new(false),
            )
            .unwrap();
            assert!(editor.save_raster_encoded(path.clone(), format, std::sync::Arc::new(bytes)));
            wait_for_files(&mut editor, &ctx);
            assert_eq!(editor.path, Some(path.clone()));
            assert_eq!(editor.document.layers().len(), 1);
            assert_eq!(
                editor.document.pixels(),
                files::open(&path).unwrap().pixels()
            );
            assert!(!editor.document.dirty());
            assert!(editor.document.undo());
            assert!(editor.document.layers() == before);
            assert!(editor.document.dirty());
            assert!(editor.document.redo());
            assert!(!editor.document.dirty());
        }
    }

    #[test]
    fn raster_save_cancel_or_failure_preserves_document_and_pending_close() {
        let ctx = egui::Context::default();
        let directory = tempfile::tempdir().unwrap();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(4, 3, [255; 4]).unwrap();
        editor.document.invert();
        let path = directory.path().join("image.png");
        editor.path = Some(path.clone());
        let before = editor.document.layers().to_vec();
        assert!(editor.save(false));
        assert!(editor.export_dialog.is_some());
        editor.after_save = Some(Action::Close);
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
        assert!(editor.export_dialog.is_none());
        assert!(matches!(editor.pending, Some(Action::Close)));
        assert!(!editor.allow_close);
        assert!(!path.exists());
        assert!(editor.document.layers() == before);
        let bytes = files::encode_raster_bytes(
            &editor.document.composite(),
            (4, 3),
            files::RasterOptions::new(files::RasterFormat::Png),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
        editor.pending = None;
        editor.after_save = Some(Action::Close);
        assert!(editor.save_raster_encoded(
            directory.path().join("missing/image.png"),
            files::RasterFormat::Png,
            std::sync::Arc::new(bytes)
        ));
        wait_for_files(&mut editor, &ctx);
        assert!(editor.document.dirty());
        assert!(editor.document.layers() == before);
        assert_eq!(editor.path, Some(path));
        assert!(matches!(editor.pending, Some(Action::Close)));
        assert!(!editor.allow_close);
    }

    #[test]
    fn save_all_serializes_dirty_documents_and_stops_on_error() {
        let ctx = egui::Context::default();
        let directory = tempfile::tempdir().unwrap();
        let mut editor = Editor::blank(&ctx);
        for index in 0..3 {
            let path = directory.path().join(format!("image{index}.prust"));
            editor.install_document(
                Document::new(2, 2, [index * 50, 10, 20, 255]).unwrap(),
                Some(path),
            );
            editor.document.invert();
        }
        editor.request(Action::SaveAll, &ctx);
        wait_for_files(&mut editor, &ctx);
        for index in 0..3 {
            let path = directory.path().join(format!("image{index}.prust"));
            assert_eq!(
                files::open(&path).unwrap().pixels()[0],
                [255 - index * 50, 245, 235, 255]
            );
        }
        assert_eq!(editor.status, "All documents are saved");
        assert!(editor.after_save.is_none());
        editor.document.invert();
        editor.path = Some(directory.path().join("missing/error.prust"));
        editor.request(Action::SaveAll, &ctx);
        wait_for_files(&mut editor, &ctx);
        assert!(editor.document.dirty());
        assert!(editor.after_save.is_none());
        assert!(editor.pending.is_none());
        assert!(editor.status.starts_with("Could not save project:"));
        assert!(editor.errors.has_entries());
    }

    #[test]
    fn save_all_raster_confirmation_can_cancel_the_remaining_sequence() {
        let ctx = egui::Context::default();
        let directory = tempfile::tempdir().unwrap();
        let mut editor = Editor::blank(&ctx);
        let project = directory.path().join("later.prust");
        editor.install_document(
            Document::new(2, 2, [255; 4]).unwrap(),
            Some(project.clone()),
        );
        editor.document.invert();
        let raster = directory.path().join("first.png");
        editor.install_document(Document::new(2, 2, [255; 4]).unwrap(), Some(raster.clone()));
        editor.document.invert();
        editor.request(Action::SaveAll, &ctx);
        assert!(editor.export_dialog.is_some());
        assert!(matches!(editor.after_save, Some(Action::SaveAll)));
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
        assert!(editor.after_save.is_none());
        assert!(editor.pending.is_none());
        assert!(editor.file_job.is_none());
        assert!(editor.document.dirty());
        assert!(!raster.exists() && !project.exists());
    }

    #[test]
    fn rulers_reserve_nonpainting_space_and_selection_fit_changes_only_view() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![]);
        let origin = editor.canvas_viewport.min;
        editor.preferences.rulers = true;
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![]);
        assert!(editor.canvas_viewport.left() >= origin.x + 40.0);
        assert!(editor.canvas_viewport.top() >= origin.y + 20.0);
        let point = Pos2::new(
            editor.canvas_viewport.left() + 50.0,
            editor.canvas_viewport.top() - 10.0,
        );
        for pressed in [true, false] {
            frame(
                &mut editor,
                &ctx,
                vec![egui::Event::PointerMoved(point), pointer(point, pressed)],
            );
        }
        assert!(!editor.document.dirty());
        editor
            .document
            .set_selection(Some(Selection::elliptical(100, 80, 20, 10)));
        let pixels = editor.document.shared_pixels();
        editor.zoom_to_selection();
        frame(&mut editor, &ctx, vec![]);
        assert!(20.0 * editor.zoom <= editor.canvas_viewport.width() + 1.0);
        assert!(10.0 * editor.zoom <= editor.canvas_viewport.height() + 1.0);
        assert_eq!(editor.document.pixels(), pixels.as_slice());
        assert!(!editor.document.dirty());
    }

    fn wait_for_files(editor: &mut Editor, ctx: &egui::Context) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while editor.file_job.is_some() {
            editor.poll_file_job(ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

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
    fn active_stroke_keeps_pending_preview_and_requests_latest_when_finished() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document.begin("Stroke");
        editor
            .document
            .stroke((1.0, 1.0), (5.0, 5.0), 2.0, [0; 4], false);
        // Model a worker that has not completed at the next UI frame.
        editor.canvas_renderer.pending = true;
        editor.texture_dirty = true;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| editor.canvas(ui));
        });
        assert!(editor.texture_dirty);
        editor.document.commit();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| editor.canvas(ui));
        });
        assert!(!editor.texture_dirty);
        assert!(editor.canvas_renderer.pending);
    }

    #[test]
    fn background_document_edits_preserve_selection_and_undo() {
        for edit in [
            DocumentEdit::Invert,
            DocumentEdit::Flatten,
            DocumentEdit::MergeDown,
            DocumentEdit::Delete,
        ] {
            let ctx = egui::Context::default();
            let mut editor = Editor::blank(&ctx);
            editor.document = Document::new(8, 8, [10, 20, 30, 255]).unwrap();
            if matches!(edit, DocumentEdit::Flatten | DocumentEdit::MergeDown) {
                editor.document.add_layer(false);
            }
            let selection = Selection {
                x: 2,
                y: 2,
                width: 3,
                height: 3,
                ellipse: None,
                mask: None,
            };
            editor.document.set_selection(Some(selection.clone()));
            let before = editor.document.snapshot();
            let count = editor.document.history().count();
            editor.start_document_edit(edit);
            assert!(editor.document.layers() == before.layers());
            wait_for_files(&mut editor, &ctx);
            assert_eq!(
                editor.document.selection(),
                if edit == DocumentEdit::Delete {
                    None
                } else {
                    Some(selection.clone())
                }
            );
            assert_eq!(editor.document.history().count(), count + 1);
            if matches!(edit, DocumentEdit::Flatten | DocumentEdit::MergeDown) {
                assert_eq!(editor.document.layers().len(), 1);
                assert_eq!(editor.document.composite(), before.composite());
            } else {
                assert_eq!(editor.document.pixels()[0], [10, 20, 30, 255]);
                assert_eq!(
                    editor.document.pixels()[3 * 8 + 3],
                    if edit == DocumentEdit::Delete {
                        [0; 4]
                    } else {
                        [245, 235, 225, 255]
                    }
                );
            }
            editor.undo();
            assert!(editor.document.layers() == before.layers());
            assert_eq!(editor.document.selection(), Some(selection));
        }
    }

    #[test]
    fn secondary_drawing_tools_commit_once_and_escape_restores_pixels() {
        for tool in [
            Tool::Brush,
            Tool::Pencil,
            Tool::Line,
            Tool::Rectangle,
            Tool::Ellipse,
            Tool::Shapes,
            Tool::Gradient,
        ] {
            for cancel in [false, true] {
                let ctx = egui::Context::default();
                let mut editor = Editor::blank(&ctx);
                editor.tool = tool;
                editor.color = Color32::BLACK.into();
                editor.secondary_color = Color32::RED.into();
                frame(&mut editor, &ctx, vec![]);
                let start = Pos2::new(350.0, 250.0);
                let end = Pos2::new(390.0, 280.0);
                let button = |pos, pressed| egui::Event::PointerButton {
                    pos,
                    pressed,
                    button: egui::PointerButton::Secondary,
                    modifiers: egui::Modifiers::NONE,
                };
                frame(
                    &mut editor,
                    &ctx,
                    vec![egui::Event::PointerMoved(start), button(start, true)],
                );
                frame(&mut editor, &ctx, vec![egui::Event::PointerMoved(end)]);
                if cancel {
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
                    // Continuing to hold the button must not restart a cancelled gesture.
                    frame(&mut editor, &ctx, vec![egui::Event::PointerMoved(start)]);
                }
                frame(&mut editor, &ctx, vec![button(end, false)]);
                wait_for_files(&mut editor, &ctx);
                if cancel {
                    assert_eq!(editor.document.history().count(), 0);
                } else {
                    assert_eq!(editor.document.history().count(), 1);
                    assert!(editor.document.pixels().iter().any(|p| p[0] > p[1]));
                    editor.undo();
                }
                assert!(editor.document.pixels().iter().all(|p| *p == [255; 4]));
                assert_eq!(editor.color, Color32::BLACK);
                assert_eq!(editor.secondary_color, Color32::RED);
            }
        }
    }

    #[test]
    fn secondary_picker_updates_only_secondary_and_text_uses_secondary() {
        for tool in [Tool::Picker, Tool::Text] {
            let ctx = egui::Context::default();
            let mut editor = Editor::blank(&ctx);
            editor.tool = tool;
            editor.color = Color32::BLACK.into();
            editor.secondary_color = Color32::RED.into();
            frame(&mut editor, &ctx, vec![]);
            let pos = Pos2::new(350.0, 250.0);
            for pressed in [true, false] {
                frame(
                    &mut editor,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            pressed,
                            button: egui::PointerButton::Secondary,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            assert_eq!(editor.color, Color32::BLACK);
            if tool == Tool::Picker {
                assert_eq!(editor.secondary_color, Color32::WHITE);
            } else {
                assert_eq!(editor.text_dialog.as_ref().unwrap().color, Color32::RED);
            }
            assert!(!editor.document.dirty());
        }
    }

    #[test]
    fn zoom_tool_clicks_and_region_drag_change_only_the_view() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.tool = Tool::Zoom;
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![]);
        let point = editor.canvas_viewport.min + Vec2::new(120.0, 100.0);
        for button in [egui::PointerButton::Primary, egui::PointerButton::Secondary] {
            for pressed in [true, false] {
                frame(
                    &mut editor,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            assert_eq!(
                editor.zoom,
                if button == egui::PointerButton::Primary {
                    2.0
                } else {
                    1.0
                }
            );
        }
        let end = point + Vec2::new(120.0, 80.0);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(point), pointer(point, true)],
        );
        frame(&mut editor, &ctx, vec![egui::Event::PointerMoved(end)]);
        let expected =
            (editor.canvas_viewport.width() / 120.0).min(editor.canvas_viewport.height() / 80.0);
        frame(&mut editor, &ctx, vec![pointer(end, false)]);
        assert!((editor.zoom - expected).abs() < 0.1);
        assert!(!editor.document.dirty());
        assert!(!editor.document.can_undo());
        assert!(editor.document.selection().is_none());
    }

    #[test]
    fn space_pan_can_end_before_button_release_without_painting() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(2048, 2048, [255; 4]).unwrap();
        editor.canvas_offset = Vec2::new(100.0, 100.0);
        editor.stroke_button = egui::PointerButton::Secondary;
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![]);
        let start = editor.canvas_viewport.min + Vec2::new(160.0, 100.0);
        let end = start - Vec2::new(30.0, 20.0);
        let space = |pressed| egui::Event::Key {
            key: Key::Space,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &mut editor,
            &ctx,
            vec![
                space(true),
                egui::Event::PointerMoved(start),
                pointer(start, true),
            ],
        );
        frame(&mut editor, &ctx, vec![egui::Event::PointerMoved(end)]);
        frame(&mut editor, &ctx, vec![space(false)]);
        frame(&mut editor, &ctx, vec![pointer(end, false)]);
        assert!(!editor.document.dirty());
        assert!(!editor.document.can_undo());
        assert!(editor.canvas_offset.x > 100.0 && editor.canvas_offset.y > 100.0);
        assert!(!editor.panning);
    }

    #[test]
    fn pencil_shortcut_and_both_buttons_draw_single_pixel_strokes() {
        for button in [egui::PointerButton::Primary, egui::PointerButton::Secondary] {
            let ctx = egui::Context::default();
            let mut editor = Editor::blank(&ctx);
            editor.color = Color32::BLACK.into();
            editor.secondary_color = Color32::RED.into();
            editor.brush_size = 48.0;
            frame(&mut editor, &ctx, vec![]);
            frame(
                &mut editor,
                &ctx,
                vec![egui::Event::Key {
                    key: Key::N,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            assert!(editor.tool == Tool::Pencil);
            frame(&mut editor, &ctx, vec![]);
            let start = editor.canvas_viewport.min + Vec2::new(100.0, 100.0);
            let end = start + Vec2::new(20.0, 0.0);
            let event = |pos, pressed| egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            frame(
                &mut editor,
                &ctx,
                vec![egui::Event::PointerMoved(start), event(start, true)],
            );
            frame(&mut editor, &ctx, vec![egui::Event::PointerMoved(end)]);
            assert!(editor.document.editing());
            frame(&mut editor, &ctx, vec![event(end, false)]);
            let ink = if button == egui::PointerButton::Primary {
                [0, 0, 0, 255]
            } else {
                [255, 0, 0, 255]
            };
            assert_eq!(
                editor
                    .document
                    .pixels()
                    .iter()
                    .filter(|p| **p == ink)
                    .count(),
                21
            );
            assert_eq!(editor.document.history().count(), 1);
            editor.undo();
            assert!(editor.document.pixels().iter().all(|p| *p == [255; 4]));
        }
    }

    #[test]
    fn canvas_fill_uses_the_clicked_buttons_color() {
        for (button, expected) in [
            (egui::PointerButton::Primary, [200, 30, 40, 255]),
            (egui::PointerButton::Secondary, [20, 80, 210, 255]),
        ] {
            let ctx = egui::Context::default();
            let mut editor = Editor::blank(&ctx);
            editor.tool = Tool::Fill;
            editor.color = Color32::from_rgb(200, 30, 40).into();
            editor.secondary_color = Color32::from_rgb(20, 80, 210).into();
            frame(&mut editor, &ctx, vec![]);
            let point = Pos2::new(350.0, 200.0);
            for pressed in [true, false] {
                frame(
                    &mut editor,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            wait_for_files(&mut editor, &ctx);
            assert!(
                editor
                    .document
                    .pixels()
                    .iter()
                    .all(|pixel| *pixel == expected)
            );
            assert_eq!(editor.document.history().count(), 1);
            assert_eq!(editor.color, Color32::from_rgb(200, 30, 40));
            assert_eq!(editor.secondary_color, Color32::from_rgb(20, 80, 210));
            editor.undo();
            assert!(
                editor
                    .document
                    .pixels()
                    .iter()
                    .all(|pixel| *pixel == [255; 4])
            );
        }
    }

    #[test]
    fn background_fill_preserves_selection_and_commits_one_undo_step() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(8, 8, [255; 4]).unwrap();
        let selection = Selection {
            x: 2,
            y: 2,
            width: 3,
            height: 3,
            ellipse: None,
            mask: None,
        };
        editor.document.set_selection(Some(selection.clone()));
        editor.color = Color32::BLACK.into();
        editor.start_fill(3, 3, editor.color.to_srgba_unmultiplied());
        assert!(!editor.document.dirty());
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.selection(), Some(selection.clone()));
        assert_eq!(editor.document.history().count(), 1);
        assert_eq!(editor.document.pixels()[3 * 8 + 3], [0, 0, 0, 255]);
        assert_eq!(editor.document.pixels()[0], [255; 4]);
        editor.undo();
        assert!(editor.document.pixels().iter().all(|p| *p == [255; 4]));
        assert_eq!(editor.document.selection(), Some(selection));
    }

    #[test]
    fn accessibility_tree_exposes_canvas_palette_and_layer_controls() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut editor = Editor::blank(&ctx);
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 1000.0))),
                ..Default::default()
            },
            |ctx| editor.show(ctx),
        );
        let tree = output
            .platform_output
            .accesskit_update
            .expect("accessibility tree");
        for (_, node) in &tree.nodes {
            if matches!(
                node.role(),
                egui::accesskit::Role::Slider | egui::accesskit::Role::SpinButton
            ) {
                assert!(
                    node.label().is_some() || !node.labelled_by().is_empty(),
                    "Unlabelled numeric control: {:?}",
                    node.role()
                );
            }
        }
        let labels: Vec<_> = tree
            .nodes
            .iter()
            .filter_map(|(_, node)| node.label())
            .collect();
        assert!(
            labels
                .iter()
                .any(|label| label.starts_with("Image canvas,")),
            "{labels:?}"
        );
        assert!(labels.contains(&"Untitled — Paint Rust"));
        assert!(
            labels.contains(&"Set primary color to #000000"),
            "{labels:?}"
        );
        assert!(
            labels.iter().any(|label| label.starts_with("Show layer ")),
            "{labels:?}"
        );
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
        wait_for_files(&mut editor, &ctx);
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
            mask: None,
            ellipse: None,
            x: 5,
            y: 6,
            width: 7,
            height: 8,
        };
        editor.document.set_selection(Some(previous.clone()));
        frame(&mut editor, &ctx, vec![]);
        let start = Pos2::new(350.0, 200.0);
        let end = Pos2::new(430.0, 280.0);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
        );
        frame(&mut editor, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_ne!(editor.document.selection(), Some(previous.clone()));
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
    fn selection_geometry_clipboard_roundtrip_modes_and_invalid_input_preserve_pixels() {
        struct TextClipboard(std::sync::Arc<std::sync::Mutex<String>>);
        impl crate::clipboard::Backend for TextClipboard {
            fn write(&mut self, _: u32, _: u32, _: &[[u8; 4]]) -> Result<(), String> {
                panic!("Expected text clipboard write")
            }
            fn read(&mut self) -> Result<(u32, u32, Vec<[u8; 4]>), String> {
                panic!("Expected text clipboard read")
            }
            fn write_text(&mut self, text: &str) -> Result<(), String> {
                *self.0.lock().unwrap() = text.into();
                Ok(())
            }
            fn read_text(&mut self) -> Result<String, String> {
                Ok(self.0.lock().unwrap().clone())
            }
        }
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(12, 10, [20, 30, 40, 255]).unwrap();
        let source = Selection::elliptical(2, 2, 8, 6);
        editor.document.set_selection(Some(source.clone()));
        let pixels = editor.document.shared_pixels();
        let text = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        *editor.clipboard.lock().unwrap() = Some(Box::new(TextClipboard(text.clone())));
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::Key {
                key: Key::C,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    ctrl: true,
                    command: true,
                    alt: true,
                    shift: true,
                    ..Default::default()
                },
            }],
        );
        wait_for_files(&mut editor, &ctx);
        assert!(text.lock().unwrap().contains("polygonList"));
        assert_eq!(editor.document.selection(), Some(source));
        assert!(!editor.document.dirty());
        let candidate = paint_rust::selection::clipboard::decode(
            12,
            10,
            &text.lock().unwrap(),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
        let previous = Selection::elliptical(0, 0, 6, 6);
        for mode in [
            CombineMode::Replace,
            CombineMode::Add,
            CombineMode::Subtract,
            CombineMode::Intersect,
            CombineMode::Xor,
        ] {
            editor.document.set_selection(Some(previous.clone()));
            let expected = paint_rust::selection::combine_cancellable(
                12,
                10,
                Some(&previous),
                &candidate,
                mode,
                &std::sync::atomic::AtomicBool::new(false),
            )
            .unwrap();
            editor.paste_selection_geometry(mode);
            assert_eq!(editor.document.selection(), Some(previous.clone()));
            wait_for_files(&mut editor, &ctx);
            assert_eq!(editor.document.selection(), Some(expected));
            assert_eq!(editor.document.pixels(), pixels.as_slice());
            assert!(editor.document.can_undo());
            let completed = editor.document.selection();
            editor.undo();
            assert_eq!(editor.document.selection(), Some(previous.clone()));
            editor.redo();
            assert_eq!(editor.document.selection(), completed);
            assert!(!editor.document.dirty());
        }
        let before = editor.document.selection();
        *text.lock().unwrap() = "invalid JSON".into();
        editor.paste_selection_geometry(CombineMode::Replace);
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.selection(), before);
        assert_eq!(editor.document.pixels(), pixels.as_slice());
        assert!(editor.status.contains("Invalid selection geometry"));
    }

    #[test]
    fn paste_new_image_shortcut_preserves_previous_document_and_marks_new_tab_unsaved() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(4, 4, [20, 30, 40, 255]).unwrap();
        editor.document.invert();
        editor.path = Some("original.prust".into());
        let old_id = editor.active_document_id;
        let old_pixels = editor.document.shared_pixels();
        let pixels = vec![
            [255, 0, 0, 0],
            [0, 0, 255, 37],
            [10, 20, 30, 255],
            [40, 50, 60, 128],
            [70, 80, 90, 1],
            [100, 110, 120, 254],
        ];
        *editor.clipboard.lock().unwrap() = Some(Box::new(MemoryClipboard {
            image: Some((2, 3, pixels.clone())),
            fail: false,
        }));
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::Key {
                key: Key::V,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers {
                    ctrl: true,
                    command: true,
                    alt: true,
                    ..Default::default()
                },
            }],
        );
        // A small in-memory clipboard job can finish within the triggering frame.
        assert!(editor.file_job.is_some() || editor.active_document_id != old_id);
        wait_for_files(&mut editor, &ctx);
        assert_ne!(editor.active_document_id, old_id);
        assert_eq!((editor.document.width, editor.document.height), (2, 3));
        assert_eq!(editor.document.pixels(), pixels);
        assert!(editor.document.dirty());
        assert!(!editor.document.can_undo());
        assert!(editor.path.is_none());
        assert_eq!(editor.document.layers().len(), 1);
        editor.activate_document(old_id);
        assert_eq!(editor.document.pixels(), old_pixels.as_slice());
        assert!(editor.document.dirty());
        assert_eq!(editor.document.history().count(), 1);
        assert_eq!(editor.path, Some("original.prust".into()));
    }
    #[test]
    fn cancelled_or_invalid_paste_new_image_never_installs_a_tab() {
        struct GatedRead {
            entered: std::sync::mpsc::Sender<()>,
            release: std::sync::mpsc::Receiver<()>,
        }
        impl crate::clipboard::Backend for GatedRead {
            fn write(&mut self, _: u32, _: u32, _: &[[u8; 4]]) -> Result<(), String> {
                unreachable!()
            }
            fn read(&mut self) -> Result<(u32, u32, Vec<[u8; 4]>), String> {
                self.entered.send(()).unwrap();
                self.release.recv().unwrap();
                Ok((2, 2, vec![[10, 20, 30, 255]; 4]))
            }
        }
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(4, 4, [20, 30, 40, 255]).unwrap();
        editor.document.invert();
        let old = editor.document.shared_pixels();
        let id = editor.active_document_id;
        let (entered, waiting) = std::sync::mpsc::channel();
        let (release, receiver) = std::sync::mpsc::channel();
        *editor.clipboard.lock().unwrap() = Some(Box::new(GatedRead {
            entered,
            release: receiver,
        }));
        editor.paste_new_image();
        waiting
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        editor
            .file_job
            .as_ref()
            .unwrap()
            .cancel
            .as_ref()
            .unwrap()
            .store(true, std::sync::atomic::Ordering::Relaxed);
        release.send(()).unwrap();
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.active_document_id, id);
        assert_eq!(editor.document.pixels(), old.as_slice());
        assert!(editor.documents.is_empty());
        assert_eq!(editor.status, "Cancelled: paste new image");
        *editor.clipboard.lock().unwrap() = Some(Box::new(MemoryClipboard {
            image: Some((2, 2, vec![[1; 4]])),
            fail: false,
        }));
        editor.paste_new_image();
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.active_document_id, id);
        assert!(editor.documents.is_empty());
        assert_eq!(editor.document.pixels(), old.as_slice());
        assert!(editor.status.contains("Pixel buffer"));
    }

    #[test]
    fn failed_clipboard_write_never_deletes_cut_pixels() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        *editor.clipboard.lock().unwrap() = Some(Box::new(MemoryClipboard {
            image: None,
            fail: true,
        }));
        editor.copy(true, false);
        wait_for_files(&mut editor, &ctx);
        assert!(!editor.document.dirty());
        assert!(!editor.document.can_undo());
        assert!(editor.document.pixels().iter().all(|p| *p == [255; 4]));
    }

    #[test]
    fn cancelling_cut_after_clipboard_write_preserves_pixels_and_keeps_ui_running() {
        struct GatedClipboard {
            entered: std::sync::mpsc::Sender<()>,
            release: std::sync::mpsc::Receiver<()>,
        }
        impl crate::clipboard::Backend for GatedClipboard {
            fn write(
                &mut self,
                _width: u32,
                _height: u32,
                _pixels: &[[u8; 4]],
            ) -> Result<(), String> {
                self.entered.send(()).unwrap();
                self.release.recv().unwrap();
                Ok(())
            }
            fn read(&mut self) -> Result<(u32, u32, Vec<[u8; 4]>), String> {
                unreachable!()
            }
        }
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(4, 4, [17; 4]).unwrap();
        let (entered, waiting) = std::sync::mpsc::channel();
        let (release, receiver) = std::sync::mpsc::channel();
        *editor.clipboard.lock().unwrap() = Some(Box::new(GatedClipboard {
            entered,
            release: receiver,
        }));
        editor.copy(true, false);
        waiting
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        frame(&mut editor, &ctx, vec![]);
        editor
            .file_job
            .as_ref()
            .unwrap()
            .cancel
            .as_ref()
            .unwrap()
            .store(true, std::sync::atomic::Ordering::Relaxed);
        release.send(()).unwrap();
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.pixels(), &[[17; 4]; 16]);
        assert!(!editor.document.dirty());
        assert!(!editor.document.can_undo());
        assert_eq!(editor.status, "Cancelled: cut image");
    }

    #[test]
    fn paste_shortcuts_distinguish_active_and_new_layers_and_fill_shortcuts_use_both_colors() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(3, 3, [20, 30, 40, 255]).unwrap();
        *editor.clipboard.lock().unwrap() = Some(Box::new(MemoryClipboard {
            image: Some((1, 1, vec![[255, 0, 0, 0]])),
            fail: false,
        }));
        let paste = |shift| egui::Event::Key {
            key: Key::V,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers {
                command: true,
                ctrl: true,
                shift,
                ..Default::default()
            },
        };
        frame(&mut editor, &ctx, vec![paste(false)]);
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.layers().len(), 1);
        assert_eq!(editor.document.pixels()[4], [255, 0, 0, 0]);
        assert!(editor.tool == Tool::Move);
        assert!(!editor.move_selection_only);
        frame(&mut editor, &ctx, vec![paste(true)]);
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.layers().len(), 2);
        assert_eq!(editor.document.pixels()[4], [255, 0, 0, 0]);
        assert_eq!(editor.document.history().count(), 2);
        editor.undo();
        assert_eq!(editor.document.layers().len(), 1);
        editor.undo();
        assert_eq!(editor.document.pixels(), &[[20, 30, 40, 255]; 9]);
        assert!(!editor.document.dirty());
        editor.color = Rgba::from_rgba_unmultiplied(10, 20, 30, 37);
        editor.secondary_color = Rgba::from_rgba_unmultiplied(40, 50, 60, 83);
        for (shift, pixel) in [(false, [10, 20, 30, 37]), (true, [40, 50, 60, 83])] {
            frame(
                &mut editor,
                &ctx,
                vec![egui::Event::Key {
                    key: Key::Backspace,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers {
                        shift,
                        ..Default::default()
                    },
                }],
            );
            wait_for_files(&mut editor, &ctx);
            assert_eq!(editor.document.pixels(), &[pixel; 9]);
        }
    }

    #[test]
    fn native_cut_event_and_paste_release_preserve_image_and_history() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        *editor.clipboard.lock().unwrap() = Some(Box::new(MemoryClipboard {
            image: None,
            fail: false,
        }));
        editor.document.set_selection(Some(Selection {
            mask: None,
            ellipse: None,
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        }));
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![egui::Event::Cut]);
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.pixels()[0], [255, 255, 255, 0]);
        assert!(editor.document.selection().is_none());
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
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.layers().len(), 1);
        assert!(editor.tool == Tool::Move);
        let selected = editor.document.selection().unwrap();
        assert_eq!(
            editor.document.pixels()[(selected.y * editor.document.width + selected.x) as usize],
            [255; 4]
        );
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
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !editor.text_dialog.as_ref().unwrap().preview.ready {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
            frame(&mut editor, &ctx, vec![]);
        }
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
    fn close_during_text_edit_keeps_dialog_and_does_not_commit_partial_preview() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        frame(&mut editor, &ctx, vec![]);
        editor.begin_text(&ctx, (10.0, 10.0), editor.color);
        editor.text_dialog.as_mut().unwrap().content = "Keep this draft".into();
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        let output = ctx.run(input, |ctx| editor.show(ctx));
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::CancelClose))
        );
        assert_eq!(
            editor.text_dialog.as_ref().unwrap().content,
            "Keep this draft"
        );
        assert!(editor.document.editing());
        assert!(!editor.document.can_undo());
        assert!(editor.pending.is_none());
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
        editor.effect_dialog = Some(EffectDialog {
            effect,
            preview,
            histogram: None,
            output_histogram: None,
        });
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
        editor.request(Action::CloseDocument, &ctx);
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
        wait_for_files(&mut editor, &ctx);
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
        wait_for_files(&mut editor, &ctx);
        assert!(editor.document.layers() == before);
        assert!(editor.status.starts_with("Could not import image:"));
    }
    #[test]
    fn save_completion_controls_followup_and_failure_preserves_dirty_document() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(4, 4, [255; 4]).unwrap();
        editor.document.invert();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("saved.prust");
        assert!(editor.save_to_path(path.clone()));
        editor.after_save = Some(Action::New);
        assert!(editor.document.dirty());
        assert!(!editor.new_dialog);
        wait_for_files(&mut editor, &ctx);
        assert!(editor.new_dialog);
        assert!(!editor.document.dirty());
        assert_eq!(
            files::open(&path).unwrap().pixels(),
            editor.document.pixels()
        );
        editor.new_dialog = false;
        editor.document.invert();
        assert!(editor.save_to_path(dir.path().join("missing/saved.prust")));
        editor.after_save = Some(Action::Close);
        wait_for_files(&mut editor, &ctx);
        assert!(editor.document.dirty());
        assert!(!editor.allow_close);
        assert!(matches!(editor.pending, Some(Action::Close)));
        assert_eq!(editor.path, Some(path));
        assert!(editor.status.starts_with("Could not save project:"));
        assert!(editor.errors.has_entries());
    }

    #[test]
    fn running_file_job_blocks_edits_and_replacement_until_completion() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        frame(&mut editor, &ctx, vec![]);
        let (release, wait) = std::sync::mpsc::channel();
        assert!(editor.start_file_job("test operation", move || {
            wait.recv().unwrap();
            Err("Test failure".into())
        }));
        assert!(!editor.start_file_job("second operation", || unreachable!()));
        editor.request(Action::New, &ctx);
        assert!(!editor.new_dialog);
        let point = Pos2::new(350.0, 200.0);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(point), pointer(point, true)],
        );
        frame(&mut editor, &ctx, vec![pointer(point, false)]);
        assert!(!editor.document.dirty());
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        let output = ctx.run(input, |ctx| editor.show(ctx));
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::CancelClose))
        );
        release.send(()).unwrap();
        wait_for_files(&mut editor, &ctx);
        assert!(editor.status.contains("Test failure"));
        assert!(!editor.document.dirty());
    }

    #[test]
    fn failed_background_open_preserves_current_pixels_and_path() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(2, 2, [10, 20, 30, 255]).unwrap();
        editor.path = Some(PathBuf::from("current.prust"));
        let dir = tempfile::tempdir().unwrap();
        editor.load(dir.path().join("missing.prust"));
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.pixels(), &[[10, 20, 30, 255]; 4]);
        assert_eq!(editor.path, Some(PathBuf::from("current.prust")));
        assert!(editor.status.starts_with("Could not open image:"));
    }
    #[test]
    fn zoom_keeps_point_under_pointer_and_pan_never_edits_pixels() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(1200, 1000, [255; 4]).unwrap();
        frame(&mut editor, &ctx, vec![]);
        let point = editor.canvas_viewport.min + Vec2::new(200.0, 150.0);
        let local_before =
            (point - editor.canvas_viewport.min + editor.canvas_offset) / editor.zoom;
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(point), egui::Event::Zoom(2.0)],
        );
        assert_eq!(editor.zoom, 2.0);
        let local_after = (point - editor.canvas_viewport.min + editor.canvas_offset) / editor.zoom;
        assert!((local_before - local_after).length() < 0.1);
        let offset = editor.canvas_offset;
        let middle = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Middle,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&mut editor, &ctx, vec![middle(point, true)]);
        let moved = point + Vec2::new(40.0, 30.0);
        frame(&mut editor, &ctx, vec![egui::Event::PointerMoved(moved)]);
        assert!((editor.canvas_offset - (offset - Vec2::new(40.0, 30.0))).length() < 0.1);
        frame(&mut editor, &ctx, vec![middle(moved, false)]);
        assert!(!editor.panning);
        assert!(!editor.document.dirty());
        assert!(!editor.document.can_undo());
    }

    #[test]
    fn fitting_uses_canvas_viewport_and_resets_scroll() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.zoom = 4.0;
        editor.canvas_offset = Vec2::splat(100.0);
        editor.fit_canvas = true;
        frame(&mut editor, &ctx, vec![]);
        assert!(editor.document.width as f32 * editor.zoom <= editor.canvas_viewport.width() + 1.0);
        assert!(
            editor.document.height as f32 * editor.zoom <= editor.canvas_viewport.height() + 1.0
        );
        assert_eq!(editor.canvas_offset, Vec2::ZERO);
        assert!(!editor.document.dirty());
    }
    #[test]
    fn background_geometry_is_published_once_and_failure_keeps_current_image() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(4, 3, [20, 40, 60, 255]).unwrap();
        editor.document.invert();
        editor.document.mark_saved();
        assert!(
            editor.transform_in_background("resize image", |doc, cancel| doc
                .resize_image_cancellable(8, 6, true, cancel))
        );
        assert_eq!((editor.document.width, editor.document.height), (4, 3));
        wait_for_files(&mut editor, &ctx);
        assert_eq!((editor.document.width, editor.document.height), (8, 6));
        assert_eq!(editor.document.history().next(), Some("Resize image"));
        editor.document.undo();
        assert!(!editor.document.dirty());
        assert_eq!((editor.document.width, editor.document.height), (4, 3));
        assert!(
            editor.transform_in_background("resize image", |doc, cancel| doc
                .resize_image_cancellable(0, 6, true, cancel))
        );
        wait_for_files(&mut editor, &ctx);
        assert_eq!((editor.document.width, editor.document.height), (4, 3));
        assert!(!editor.document.dirty());
        assert!(editor.status.starts_with("Could not resize image:"));
    }
    #[test]
    fn select_all_and_deselect_shortcuts_are_clean_undoable_operations() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(8, 6, [20, 30, 40, 255]).unwrap();
        let command = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
        };
        frame(&mut editor, &ctx, vec![command(Key::A)]);
        let selected = editor.document.selection();
        assert_eq!(selected.as_ref().unwrap().width, 8);
        assert_eq!(editor.document.history().count(), 1);
        assert!(!editor.document.dirty());
        frame(&mut editor, &ctx, vec![command(Key::D)]);
        assert!(editor.document.selection().is_none());
        assert_eq!(editor.document.history().count(), 2);
        assert!(!editor.document.dirty());
        editor.undo();
        assert_eq!(editor.document.selection(), selected);
        editor.undo();
        assert!(editor.document.selection().is_none());
        assert!(!editor.document.dirty());
        assert_eq!(editor.document.pixels(), &[[20, 30, 40, 255]; 48]);
    }

    #[test]
    fn elliptical_selection_gesture_has_clean_undo_and_redo() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.tool = Tool::Select;
        editor.ellipse_selection = true;
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![]);
        let start = editor.canvas_viewport.min + Vec2::new(130.0, 100.0);
        let end = start + Vec2::new(100.0, 80.0);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
        );
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(end), pointer(end, false)],
        );
        let selected = editor.document.selection().unwrap();
        assert!(selected.ellipse.is_some());
        assert_eq!((selected.width, selected.height), (100, 80));
        assert!(!editor.document.dirty());
        assert_eq!(editor.document.history().count(), 1);
        editor.undo();
        assert!(editor.document.selection().is_none());
        assert!(!editor.document.dirty());
        editor.redo();
        assert_eq!(editor.document.selection(), Some(selected));
        assert!(!editor.document.dirty());
    }
    #[test]
    fn arrow_nudges_move_copy_and_selection_bounds_with_undo() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(20, 5, [0; 4]).unwrap();
        editor.document.begin("Seed");
        editor.document.fill(0, 0, [200, 50, 30, 255]);
        editor.document.commit();
        editor.document.set_selection(Some(Selection {
            mask: None,
            ellipse: None,
            x: 1,
            y: 1,
            width: 1,
            height: 1,
        }));
        editor.document.mark_saved();
        editor.tool = Tool::Move;
        let key = |modifiers| egui::Event::Key {
            key: Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![key(egui::Modifiers::NONE)]);
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.selection().unwrap().x, 2);
        assert_eq!(editor.document.pixels()[21], [0; 4]);
        editor.document.undo();
        assert_eq!(editor.document.selection().unwrap().x, 1);
        assert!(!editor.document.dirty());
        editor.move_selection_only = true;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 800.0))),
            focused: true,
            modifiers: egui::Modifiers::SHIFT,
            events: vec![key(egui::Modifiers::SHIFT)],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| editor.show(ctx));
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.selection().unwrap().x, 11);
        assert!(!editor.document.dirty());
        editor.move_selection_only = false;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 800.0))),
            focused: true,
            modifiers: egui::Modifiers::COMMAND,
            events: vec![key(egui::Modifiers::COMMAND)],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| editor.show(ctx));
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.selection().unwrap().x, 12);
        assert_eq!(editor.document.pixels()[31], [200, 50, 30, 255]);
        assert_eq!(editor.document.pixels()[32], [200, 50, 30, 255]);
    }
    #[test]
    fn cancelled_save_keeps_document_dirty_and_does_not_run_close_followup() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(4, 4, [255; 4]).unwrap();
        editor.document.invert();
        let snapshot = editor.document.snapshot();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("saved.prust");
        std::fs::write(&path, b"previous data").unwrap();
        let worker_path = path.clone();
        let (release, wait) = std::sync::mpsc::channel();
        editor.start_cancellable_file_job("save project", move |cancel| {
            wait.recv().unwrap();
            files::save_project_cancellable(&snapshot, &worker_path, cancel)?;
            Ok(Outcome::Saved(worker_path))
        });
        editor.after_save = Some(Action::Close);
        editor
            .file_job
            .as_ref()
            .unwrap()
            .cancel
            .as_ref()
            .unwrap()
            .store(true, std::sync::atomic::Ordering::Relaxed);
        release.send(()).unwrap();
        wait_for_files(&mut editor, &ctx);
        assert!(editor.document.dirty());
        assert!(!editor.allow_close);
        assert!(matches!(editor.pending, Some(Action::Close)));
        assert!(editor.path.is_none());
        assert_eq!(std::fs::read(&path).unwrap(), b"previous data");
        assert_eq!(editor.status, "Cancelled: save project");
    }
    #[test]
    fn cancelled_background_open_keeps_document_path_and_history() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(2, 2, [10, 20, 30, 255]).unwrap();
        editor.document.invert();
        let before = editor.document.pixels().to_vec();
        editor.path = Some(PathBuf::from("current.prust"));
        let (release, wait) = std::sync::mpsc::channel();
        editor.start_cancellable_file_job("open image", move |cancel| {
            wait.recv().unwrap();
            let path = PathBuf::from("missing.prust");
            Ok(Outcome::Opened(
                Box::new(files::open_cancellable(&path, cancel)?),
                path,
            ))
        });
        editor
            .file_job
            .as_ref()
            .unwrap()
            .cancel
            .as_ref()
            .unwrap()
            .store(true, std::sync::atomic::Ordering::Relaxed);
        release.send(()).unwrap();
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.pixels(), before);
        assert_eq!(editor.path, Some(PathBuf::from("current.prust")));
        assert!(editor.document.dirty());
        assert!(editor.document.can_undo());
        assert_eq!(editor.status, "Cancelled: open image");
    }

    #[test]
    fn background_crop_uses_selection_and_restores_it_with_undo() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(8, 6, [10, 20, 30, 255]).unwrap();
        let area = Selection {
            x: 2,
            y: 1,
            width: 3,
            height: 4,
            mask: None,
            ellipse: None,
        };
        editor.document.set_selection(Some(area.clone()));
        assert!(
            editor.transform_in_background("crop image", Document::crop_to_selection_cancellable)
        );
        assert_eq!((editor.document.width, editor.document.height), (8, 6));
        wait_for_files(&mut editor, &ctx);
        assert_eq!((editor.document.width, editor.document.height), (3, 4));
        assert!(editor.document.selection().is_none());
        assert!(editor.document.undo());
        assert_eq!((editor.document.width, editor.document.height), (8, 6));
        assert_eq!(editor.document.selection(), Some(area));
    }

    #[test]
    fn cancelled_selection_job_preserves_previous_mask_without_history() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(8, 8, [255; 4]).unwrap();
        let previous = Selection::elliptical(1, 1, 4, 4);
        editor.document.set_selection(Some(previous.clone()));
        let (release, wait) = std::sync::mpsc::channel();
        editor.start_cancellable_file_job("select polygon", move |cancel| {
            wait.recv().unwrap();
            paint_rust::selection::polygon_cancellable(
                8,
                8,
                &[(0.0, 0.0), (8.0, 0.0), (4.0, 8.0)],
                cancel,
            )
            .map(Outcome::Selection)
        });
        editor
            .file_job
            .as_ref()
            .unwrap()
            .cancel
            .as_ref()
            .unwrap()
            .store(true, std::sync::atomic::Ordering::Relaxed);
        release.send(()).unwrap();
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.selection(), Some(previous));
        assert!(!editor.document.dirty());
        assert!(!editor.document.can_undo());
        assert_eq!(editor.status, "Cancelled: select polygon");
    }

    #[test]
    fn background_move_publishes_latest_pixels_and_selection_together() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        let original = vec![
            [10, 0, 0, 255],
            [20, 0, 0, 255],
            [30, 0, 0, 255],
            [40, 0, 0, 255],
        ];
        editor.document = Document::from_pixels(4, 1, original.clone()).unwrap();
        let selection = Selection {
            x: 0,
            y: 0,
            width: 2,
            height: 1,
            mask: None,
            ellipse: None,
        };
        editor.document.set_selection(Some(selection.clone()));
        editor.document.begin("Move pixels");
        editor.request_gesture_preview(Effect::Move(paint_rust::movement::Parameters {
            dx: 1,
            dy: 0,
            copy: false,
        }));
        editor.request_gesture_preview(Effect::Move(paint_rust::movement::Parameters {
            dx: 2,
            dy: 0,
            copy: false,
        }));
        editor.finish_stroke();
        assert_eq!(editor.document.pixels(), original);
        assert_eq!(editor.document.selection(), Some(selection.clone()));
        wait_for_files(&mut editor, &ctx);
        assert_eq!(
            editor.document.pixels(),
            &[[0; 4], [0; 4], [10, 0, 0, 255], [20, 0, 0, 255]]
        );
        assert_eq!(editor.document.selection().unwrap().x, 2);
        assert_eq!(editor.document.history().count(), 1);
        editor.undo();
        assert_eq!(editor.document.pixels(), original);
        assert_eq!(editor.document.selection(), Some(selection));
        assert!(!editor.document.dirty());
        editor.redo();
        assert_eq!(editor.document.selection().unwrap().x, 2);
    }

    #[test]
    fn finishing_gradient_waits_for_latest_request_and_commits_one_edit() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(4, 1, [0; 4]).unwrap();
        editor.tool = Tool::Gradient;
        let first = Effect::Gradient(paint_rust::gradient::Gradient {
            from: (0.5, 0.5),
            to: (3.5, 0.5),
            style: GradientStyle {
                blend_mode: Default::default(),
                start: [255, 0, 0, 255],
                end: [0, 0, 255, 255],
                shape: paint_rust::gradient::GradientShape::Linear,
                mode: paint_rust::gradient::GradientMode::Blend,
            },
        });
        let mut preview = crate::preview::EffectPreview::new(
            ctx.clone(),
            editor.document.shared_pixels(),
            4,
            1,
            None,
            first,
        );
        let Effect::Gradient(mut final_parameters) = first else {
            unreachable!()
        };
        final_parameters.from = (3.5, 0.5);
        final_parameters.to = (0.5, 0.5);
        let last = Effect::Gradient(final_parameters);
        preview.request(last);
        editor.document.begin("Gradient");
        editor.gesture_preview = Some(EffectDialog {
            histogram: None,
            output_histogram: None,
            effect: last,
            preview,
        });
        editor.finish_stroke();
        assert!(editor.file_job.is_some());
        assert_eq!(editor.document.pixels(), &[[0; 4]; 4]);
        wait_for_files(&mut editor, &ctx);
        assert_eq!(
            editor.document.pixels(),
            &[
                [0, 0, 255, 255],
                [85, 0, 170, 255],
                [170, 0, 85, 255],
                [255, 0, 0, 255]
            ]
        );
        assert_eq!(editor.document.history().count(), 1);
        editor.undo();
        assert_eq!(editor.document.pixels(), &[[0; 4]; 4]);
        assert!(!editor.document.dirty());
    }

    #[test]
    fn escape_during_drawing_gesture_discards_worker_and_pixels() {
        for tool in [
            Tool::Gradient,
            Tool::Line,
            Tool::Rectangle,
            Tool::Ellipse,
            Tool::Shapes,
            Tool::Move,
        ] {
            let ctx = egui::Context::default();
            let mut editor = Editor::blank(&ctx);
            editor.tool = tool;
            frame(&mut editor, &ctx, vec![]);
            frame(&mut editor, &ctx, vec![]);
            let from = editor.canvas_viewport.min + Vec2::new(130.0, 100.0);
            let to = from + Vec2::new(80.0, 40.0);
            frame(
                &mut editor,
                &ctx,
                vec![egui::Event::PointerMoved(from), pointer(from, true)],
            );
            frame(&mut editor, &ctx, vec![egui::Event::PointerMoved(to)]);
            assert!(editor.gesture_preview.is_some());
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
            frame(&mut editor, &ctx, vec![pointer(to, false)]);
            assert!(editor.gesture_preview.is_none());
            assert!(editor.file_job.is_none());
            assert!(!editor.document.editing());
            assert!(!editor.document.dirty());
            assert!(!editor.document.can_undo());
            assert!(
                editor
                    .document
                    .pixels()
                    .iter()
                    .all(|pixel| *pixel == [255; 4])
            );
        }
    }

    #[test]
    fn cancelled_background_geometry_never_publishes_the_worker_image() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(4, 3, [10, 20, 30, 255]).unwrap();
        let (release, wait) = std::sync::mpsc::channel();
        assert!(
            editor.transform_in_background("resize image", move |document, cancel| {
                wait.recv().unwrap();
                document.resize_image_cancellable(8, 6, true, cancel)
            })
        );
        editor
            .file_job
            .as_ref()
            .unwrap()
            .cancel
            .as_ref()
            .unwrap()
            .store(true, std::sync::atomic::Ordering::Relaxed);
        release.send(()).unwrap();
        wait_for_files(&mut editor, &ctx);
        assert_eq!((editor.document.width, editor.document.height), (4, 3));
        assert_eq!(editor.document.pixels(), &[[10, 20, 30, 255]; 12]);
        assert!(!editor.document.dirty());
        assert!(!editor.document.can_undo());
        assert_eq!(editor.status, "Cancelled: resize image");
    }

    #[test]
    fn large_history_storage_and_navigation_publish_only_completed_work() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(1024, 1024, [255; 4]).unwrap();
        editor.document.defer_history_storage(true);
        for _ in 0..35 {
            editor.document.invert();
        }
        assert!(editor.document.history_needs_storage());
        assert_eq!(editor.document.history_disk_bytes(), 0);
        let (release, wait) = std::sync::mpsc::channel();
        editor.start_cancellable_file_job("store undo history", move |_| {
            wait.recv().unwrap();
            Err(files::CANCELLED.into())
        });
        release.send(()).unwrap();
        wait_for_files(&mut editor, &ctx);
        assert!(editor.history_storage_paused);
        frame(&mut editor, &ctx, vec![]);
        assert!(editor.file_job.is_none());
        assert!(editor.document.history_needs_storage());
        editor.store_history_async();
        assert!(!editor.history_storage_paused);
        assert!(editor.file_job.is_some());
        assert_eq!(editor.document.history_disk_bytes(), 0);
        wait_for_files(&mut editor, &ctx);
        assert!(!editor.document.history_needs_storage());
        assert!(editor.document.history_disk_bytes() > 0);
        assert_eq!(editor.document.history().count(), 35);
        editor.navigate_history(35, false);
        assert!(editor.file_job.is_some());
        assert_eq!(editor.document.pixels()[0], [0, 0, 0, 255]);
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.pixels()[0], [255; 4]);
        assert!(!editor.document.dirty());
        assert_eq!(editor.document.redo_history().count(), 35);
        editor.navigate_history(35, true);
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.pixels()[0], [0, 0, 0, 255]);
        assert!(editor.document.dirty());
    }

    #[test]
    fn history_navigation_restores_saved_revision_and_redoes_in_order() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(3, 2, [10, 20, 30, 255]).unwrap();
        editor.document.invert();
        editor.document.mark_saved();
        editor.document.add_layer(false);
        editor.document.set_layer_properties("Detail".into(), 100);
        editor.navigate_history(2, false);
        assert!(!editor.document.dirty());
        assert_eq!(editor.document.layers().len(), 1);
        assert_eq!(
            editor.document.redo_history().collect::<Vec<_>>(),
            ["Add layer", "Layer properties"]
        );
        editor.navigate_history(2, true);
        assert!(editor.document.dirty());
        assert_eq!(editor.document.layers()[1].name, "Detail");
        assert_eq!(editor.document.layers()[1].opacity, 100);
        editor.navigate_history(usize::MAX, false);
        assert_eq!(editor.document.pixels(), &[[10, 20, 30, 255]; 6]);
        assert!(!editor.document.can_undo());
        editor.document.invert();
        assert!(!editor.document.can_redo());
    }
    #[test]
    fn additive_selection_gesture_publishes_one_clean_history_step() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(100, 100, [255; 4]).unwrap();
        editor.document.set_selection(Some(Selection {
            mask: None,
            ellipse: None,
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        }));
        editor.tool = Tool::Select;
        editor.selection_mode = CombineMode::Add;
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![]);
        let start = editor.canvas_viewport.min + Vec2::new(10.0, 10.0);
        let end = start + Vec2::new(20.0, 20.0);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
        );
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(end), pointer(end, false)],
        );
        wait_for_files(&mut editor, &ctx);
        frame(&mut editor, &ctx, vec![]);
        assert!(editor.document.selection().unwrap().mask.is_some());
        assert!(editor.selection_texture.is_some());
        let (_, _, copied) = editor.document.copy_pixels(false);
        assert_eq!(copied.iter().filter(|p| p[3] > 0).count(), 401);
        assert!(!editor.document.dirty());
        assert_eq!(editor.document.history().count(), 1);
        editor.undo();
        assert_eq!(editor.document.selection().unwrap().width, 1);
        assert!(!editor.document.dirty());
        editor.redo();
        assert!(editor.document.selection().unwrap().mask.is_some());
    }

    #[test]
    fn zero_size_additive_gesture_preserves_previous_selection() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        let previous = Selection::elliptical(5, 5, 10, 10);
        editor.tool = Tool::Select;
        editor.selection_mode = CombineMode::Add;
        editor.selection_before = Some(Some(previous.clone()));
        editor.document.set_selection(None);
        editor.finish_stroke();
        assert_eq!(editor.document.selection(), Some(previous));
        assert!(editor.file_job.is_none());
    }

    #[test]
    fn close_is_cancelled_when_focus_loss_starts_selection_computation() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(4, 4, [255; 4]).unwrap();
        editor.document.invert();
        editor.tool = Tool::Select;
        editor.selection_mode = CombineMode::Add;
        editor.selection_before = Some(None);
        editor
            .document
            .set_selection(Some(Selection::elliptical(0, 0, 3, 3)));
        let mut input = egui::RawInput {
            focused: false,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        let output = ctx.run(input, |ctx| editor.show(ctx));
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::CancelClose))
        );
        wait_for_files(&mut editor, &ctx);
        assert!(editor.document.dirty());
        assert!(!editor.allow_close);
    }
    #[test]
    fn polygon_clicks_enter_and_escape_preserve_image_and_previous_selection() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(100, 100, [255; 4]).unwrap();
        editor.tool = Tool::Polygon;
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![]);
        let origin = editor.canvas_viewport.min;
        for delta in [
            Vec2::new(10.0, 10.0),
            Vec2::new(50.0, 10.0),
            Vec2::new(50.0, 50.0),
        ] {
            let point = origin + delta;
            frame(
                &mut editor,
                &ctx,
                vec![egui::Event::PointerMoved(point), pointer(point, true)],
            );
            frame(&mut editor, &ctx, vec![pointer(point, false)]);
        }
        assert_eq!(editor.polygon_points.len(), 3);
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&mut editor, &ctx, vec![key(Key::Enter)]);
        wait_for_files(&mut editor, &ctx);
        let previous = editor.document.selection();
        assert!(previous.as_ref().unwrap().mask.is_some());
        assert!(!editor.document.dirty());
        assert!(editor.polygon_points.is_empty());
        editor.polygon_points.push((3.0, 3.0));
        frame(&mut editor, &ctx, vec![key(Key::Escape)]);
        assert!(editor.polygon_points.is_empty());
        assert_eq!(editor.document.selection(), previous);
        assert!(editor.document.can_undo());
    }
    #[test]
    fn layer_and_fill_commands_publish_in_background_with_single_undo() {
        let ctx = egui::Context::default();
        for edit in [
            DocumentEdit::OrientLayer(Orientation::FlipHorizontal),
            DocumentEdit::OrientLayer(Orientation::FlipVertical),
            DocumentEdit::OrientLayer(Orientation::HalfTurn),
            DocumentEdit::Fill([10, 20, 30, 37]),
        ] {
            let mut editor = Editor::blank(&ctx);
            let pixels: Vec<_> = (1..=6).map(|i| [i * 20, i * 10, i, 255]).collect();
            editor.document = Document::from_pixels(3, 2, pixels.clone()).unwrap();
            let selection = Selection::elliptical(0, 0, 1, 1);
            editor.document.set_selection(Some(selection.clone()));
            editor.start_document_edit(edit);
            assert!(editor.file_job.is_some());
            assert_eq!(editor.document.pixels(), pixels);
            assert!(!editor.document.can_undo());
            wait_for_files(&mut editor, &ctx);
            assert_ne!(editor.document.pixels(), pixels);
            assert_eq!(editor.document.history().count(), 1);
            assert_eq!(editor.document.selection(), Some(selection.clone()));
            editor.undo();
            assert_eq!(editor.document.pixels(), pixels);
            assert_eq!(editor.document.selection(), Some(selection));
            assert!(!editor.document.dirty());
        }
    }

    #[test]
    fn generated_textures_preview_cancel_apply_undo_and_repeat() {
        let ctx = egui::Context::default();
        for effect in [
            Effect::Clouds(Default::default()),
            Effect::Turbulence(Default::default()),
            Effect::Mandelbrot(Default::default()),
            Effect::Julia(Default::default()),
        ] {
            let mut editor = Editor::blank(&ctx);
            editor.document = Document::new(12, 10, [25, 50, 100, 128]).unwrap();
            editor
                .document
                .set_selection(Some(Selection::elliptical(2, 2, 8, 6)));
            let source = editor.document.pixels().to_vec();
            let selection = editor.document.selection();
            for cancel in [true, false] {
                editor.begin_effect(&ctx, effect);
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while !editor.effect_dialog.as_ref().unwrap().preview.ready {
                    frame(&mut editor, &ctx, vec![]);
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                assert_ne!(editor.document.pixels(), source);
                if cancel {
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
                    assert_eq!(editor.document.pixels(), source);
                    assert!(!editor.document.can_undo());
                    assert_eq!(editor.last_effect, None);
                } else {
                    let preview = editor.document.pixels().to_vec();
                    editor.apply_effect(effect);
                    editor.effect_dialog = None;
                    assert_eq!(editor.document.history().count(), 1);
                    assert_eq!(editor.last_effect, Some(effect));
                    editor.undo();
                    assert_eq!(editor.document.pixels(), source);
                    editor.repeat_effect();
                    wait_for_files(&mut editor, &ctx);
                    assert_eq!(editor.document.pixels(), preview);
                    assert_eq!(editor.document.history().count(), 1);
                }
                assert_eq!(editor.document.selection(), selection);
            }
        }
    }

    #[test]
    fn free_transform_preview_cancels_and_applies_as_one_undoable_edit() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(4, 4, [200, 100, 50, 255]).unwrap();
        let selection = Selection::elliptical(0, 0, 2, 2);
        editor.document.set_selection(Some(selection.clone()));
        let source = editor.document.pixels().to_vec();
        let effect = Effect::FreeTransform(paint_rust::affine::FreeTransform {
            offset_x: 2.0,
            smooth: false,
            ..Default::default()
        });
        for cancel in [true, false] {
            editor.begin_effect(&ctx, effect);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                frame(&mut editor, &ctx, vec![]);
                if editor.effect_dialog.as_ref().unwrap().preview.ready {
                    break;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            assert!(editor.document.editing());
            assert_ne!(editor.document.pixels(), source);
            if cancel {
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
                assert_eq!(editor.document.pixels(), source);
                assert_eq!(editor.document.selection(), Some(selection.clone()));
                assert!(!editor.document.dirty());
            } else {
                editor.apply_effect(effect);
                editor.effect_dialog = None;
                assert!(editor.document.selection().is_none());
                assert_eq!(editor.document.history().count(), 1);
                editor.undo();
                assert_eq!(editor.document.pixels(), source);
                assert_eq!(editor.document.selection(), Some(selection.clone()));
            }
        }
    }
    #[test]
    fn transform_box_drag_updates_preview_without_drawing_a_brush_stroke() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.begin_effect(&ctx, Effect::FreeTransform(Default::default()));
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![]);
        let point = editor.canvas_viewport.min + Vec2::new(60.0, 500.0);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(point), pointer(point, true)],
        );
        assert!(editor.transform_drag.is_some());
        let end = point + Vec2::new(30.0, 20.0);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(end), pointer(end, false)],
        );
        assert!(editor.transform_drag.is_none());
        let Effect::FreeTransform(parameters) = editor.effect_dialog.as_ref().unwrap().effect
        else {
            panic!("wrong preview")
        };
        assert_eq!((parameters.offset_x, parameters.offset_y), (30.0, 20.0));
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
        assert!(!editor.document.dirty());
        assert!(
            editor
                .document
                .pixels()
                .iter()
                .all(|pixel| *pixel == [255; 4])
        );
    }
    #[test]
    fn color_selection_samples_requested_scope_and_combines_without_editing_pixels() {
        use paint_rust::document::Layer;
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        let bottom = Layer::new("Bottom".into(), vec![[255; 4]; 3]);
        let top = Layer::new("Top".into(), vec![[255; 4], [0; 4], [0, 0, 0, 255]]);
        editor.document = Document::from_layers(3, 1, vec![bottom, top], 1).unwrap();
        let original = editor.document.layers().to_vec();
        editor.start_color_selection(0, 0);
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.selection().unwrap().width, 1);
        editor.color_sample_merged = true;
        editor.start_color_selection(0, 0);
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.selection().unwrap().width, 2);
        editor.selection_mode = CombineMode::Add;
        editor.start_color_selection(2, 0);
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.selection().unwrap().width, 3);
        editor.selection_mode = CombineMode::Subtract;
        editor.start_color_selection(0, 0);
        wait_for_files(&mut editor, &ctx);
        let selection = editor.document.selection().unwrap();
        assert_eq!((selection.x, selection.width), (2, 1));
        editor.start_color_selection(3, 0);
        assert!(editor.file_job.is_none());
        assert!(editor.document.layers() == original);
        assert!(!editor.document.dirty());
        assert_eq!(editor.document.history().count(), 4);
        editor.navigate_history(4, false);
        assert!(editor.document.selection().is_none());
        assert!(!editor.document.dirty());
        editor.navigate_history(4, true);
        assert_eq!(editor.document.selection(), Some(selection));
    }
    #[test]
    fn lasso_drag_closes_on_release_and_escape_preserves_previous_selection() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(100, 100, [255; 4]).unwrap();
        editor.tool = Tool::Lasso;
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![]);
        let origin = editor.canvas_viewport.min;
        let start = origin + Vec2::new(10.0, 10.0);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
        );
        for delta in [
            Vec2::new(50.0, 10.0),
            Vec2::new(50.0, 50.0),
            Vec2::new(10.0, 50.0),
        ] {
            frame(
                &mut editor,
                &ctx,
                vec![egui::Event::PointerMoved(origin + delta)],
            );
        }
        let end = origin + Vec2::new(10.0, 50.0);
        frame(&mut editor, &ctx, vec![pointer(end, false)]);
        wait_for_files(&mut editor, &ctx);
        let selection = editor.document.selection().unwrap();
        assert_eq!(
            (selection.x, selection.y, selection.width, selection.height),
            (10, 10, 40, 40)
        );
        assert!(
            selection
                .mask
                .as_ref()
                .unwrap()
                .pixels
                .iter()
                .all(|p| *p == 1)
        );
        assert!(!editor.document.dirty());
        assert!(editor.document.can_undo());
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
        );
        frame(&mut editor, &ctx, vec![egui::Event::PointerMoved(end)]);
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
        assert!(editor.file_job.is_none());
        assert_eq!(editor.document.selection(), Some(selection.clone()));
        // A click does not turn an existing selection into an empty mask.
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
        );
        frame(&mut editor, &ctx, vec![pointer(start, false)]);
        assert!(editor.file_job.is_none());
        assert_eq!(editor.document.selection(), Some(selection));
    }
    #[test]
    fn loading_a_font_updates_only_the_open_text_draft_and_failure_keeps_the_font() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        frame(&mut editor, &ctx, vec![]);
        editor.begin_text(&ctx, (10.0, 10.0), Rgba::BLACK);
        editor.text_dialog.as_mut().unwrap().content = "Keep draft".into();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chosen.ttf");
        let definitions = egui::FontDefinitions::default();
        let name = &definitions.families[&egui::FontFamily::Proportional][0];
        std::fs::write(&path, definitions.font_data[name].font.as_ref()).unwrap();
        editor.load_text_font(path.clone());
        wait_for_files(&mut editor, &ctx);
        let dialog = editor.text_dialog.as_ref().unwrap();
        assert_eq!(dialog.font_name, "chosen.ttf");
        assert_eq!(dialog.content, "Keep draft");
        assert!(editor.text_font.is_some());
        assert!(editor.document.editing());
        assert!(!editor.document.can_undo());
        std::fs::write(&path, b"invalid").unwrap();
        editor.load_text_font(path.clone());
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.text_dialog.as_ref().unwrap().font_name, "chosen.ttf");
        assert_eq!(editor.text_dialog.as_ref().unwrap().content, "Keep draft");
        assert!(editor.status.contains("invalid font"));
        // A late worker result must not recreate a dismissed text editor.
        std::fs::write(&path, definitions.font_data[name].font.as_ref()).unwrap();
        editor.load_text_font(path);
        editor.text_dialog = None;
        editor.document.cancel();
        wait_for_files(&mut editor, &ctx);
        assert!(editor.text_dialog.is_none());
        assert!(!editor.document.dirty());
    }
    #[test]
    fn repeating_effect_preserves_selection_and_cancelled_settings_do_not_replace_parameters() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(2, 1, [80, 100, 120, 255]).unwrap();
        let selection = Selection {
            x: 1,
            y: 0,
            width: 1,
            height: 1,
            mask: None,
            ellipse: None,
        };
        editor.document.set_selection(Some(selection.clone()));
        let effect = Effect::BrightnessContrast {
            brightness: 20,
            contrast: 0,
        };
        editor.begin_effect(&ctx, effect);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !editor.effect_dialog.as_ref().unwrap().preview.ready {
            frame(&mut editor, &ctx, vec![]);
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        editor.apply_effect(effect);
        editor.effect_dialog = None;
        let once = editor.document.pixels().to_vec();
        assert_eq!(editor.last_effect, Some(effect));
        editor.repeat_effect();
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.pixels()[0], [80, 100, 120, 255]);
        assert!(editor.document.pixels()[1][0] > once[1][0]);
        assert_eq!(editor.document.history().count(), 2);
        assert_eq!(editor.document.selection(), Some(selection));
        editor.undo();
        assert_eq!(editor.document.pixels(), once);
        editor.begin_effect(
            &ctx,
            Effect::BrightnessContrast {
                brightness: 0,
                contrast: 0,
            },
        );
        assert_eq!(editor.effect_dialog.as_ref().unwrap().effect, effect);
        editor.effect_dialog.as_mut().unwrap().effect = Effect::BrightnessContrast {
            brightness: 90,
            contrast: 0,
        };
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
        assert_eq!(editor.last_effect, Some(effect));
        assert_eq!(
            editor.effect_parameters[&std::mem::discriminant(&effect)],
            effect
        );
        assert_eq!(editor.document.pixels(), once);
        editor.apply_effect(Effect::FreeTransform(Default::default()));
        assert_eq!(editor.last_effect, Some(effect));
    }
    #[test]
    fn selection_inversion_keeps_empty_distinct_from_deselected_and_leaves_pixels_intact() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(5, 5, [255; 4]).unwrap();
        editor.invert_selection();
        wait_for_files(&mut editor, &ctx);
        assert_eq!(editor.document.selection().unwrap().width, 0);
        editor.invert_selection();
        wait_for_files(&mut editor, &ctx);
        assert_eq!(
            editor
                .document
                .selection()
                .unwrap()
                .mask
                .unwrap()
                .pixels
                .as_ref(),
            &vec![1; 25]
        );
        editor
            .document
            .set_selection(Some(Selection::elliptical(0, 0, 5, 5)));
        editor.invert_selection();
        wait_for_files(&mut editor, &ctx);
        let mask = editor.document.selection().unwrap().mask.unwrap();
        assert_eq!(
            mask.pixels.as_ref(),
            &vec![
                1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1
            ]
        );
        editor.invert_selection();
        wait_for_files(&mut editor, &ctx);
        assert_eq!(
            editor
                .document
                .selection()
                .unwrap()
                .mask
                .unwrap()
                .pixels
                .as_ref(),
            &vec![
                0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 0
            ]
        );
        assert_eq!(editor.document.pixels(), &[[255; 4]; 25]);
        assert!(!editor.document.dirty());
        assert!(editor.document.can_undo());
    }
    #[test]
    fn export_preview_guards_document_switching_and_cancel_does_not_write_or_edit() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.install_document(Document::new(2, 2, [255; 4]).unwrap(), None);
        editor.install_document(Document::new(3, 3, [100, 120, 140, 128]).unwrap(), None);
        let active = editor.active_document_id;
        let before = editor.document.pixels().to_vec();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preview.jpg");
        editor.begin_export(path.clone(), files::RasterFormat::Jpeg);
        editor.activate_document(0);
        assert_eq!(editor.active_document_id, active);
        frame(&mut editor, &ctx, vec![]);
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
        assert!(editor.export_dialog.is_none());
        assert!(!path.exists());
        assert_eq!(editor.document.pixels(), before);
        assert!(!editor.document.dirty());
        assert!(!editor.document.can_undo());
        editor.activate_document(0);
        assert_eq!(editor.active_document_id, 0);
    }
    #[test]
    fn editable_shape_release_waits_for_explicit_apply_or_cancel() {
        for apply in [false, true] {
            let ctx = egui::Context::default();
            let mut editor = Editor::blank(&ctx);
            editor.tool = Tool::Rectangle;
            editor.shape_filled = true;
            editor.keep_shape_editable = true;
            frame(&mut editor, &ctx, vec![]);
            let start = editor.canvas_viewport.min + Vec2::new(50.0, 50.0);
            let end = start + Vec2::new(80.0, 60.0);
            frame(
                &mut editor,
                &ctx,
                vec![egui::Event::PointerMoved(start), pointer(start, true)],
            );
            frame(
                &mut editor,
                &ctx,
                vec![egui::Event::PointerMoved(end), pointer(end, false)],
            );
            assert!(editor.shape_edit.is_some());
            assert!(editor.document.editing());
            assert!(editor.file_job.is_none());
            frame(&mut editor, &ctx, vec![]);
            assert!(editor.shape_edit.is_some());
            assert_eq!(editor.document.history().count(), 0);
            editor.finish_shape_edit(apply);
            wait_for_files(&mut editor, &ctx);
            assert!(editor.shape_edit.is_none());
            assert!(!editor.document.editing());
            assert_eq!(editor.document.history().count(), usize::from(apply));
            if apply {
                editor.undo();
            }
            assert!(
                editor
                    .document
                    .pixels()
                    .iter()
                    .all(|pixel| *pixel == [255; 4])
            );
            assert!(!editor.document.dirty());
        }
    }
}
