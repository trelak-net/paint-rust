use super::*;
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum DocumentEdit {
    OrientLayer(Orientation),
    Fill([u8; 4]),
    Flatten,
    MergeDown,
    Invert,
    Delete,
}

pub(super) enum Outcome {
    SelectionCopied,
    PastedImage(Box<Document>),
    TextFont(ab_glyph::FontArc, String),
    FontCatalog(Vec<paint_rust::fonts::InstalledFont>),
    Copied(Option<Box<Document>>),
    Opened(Box<Document>, PathBuf),
    Recovered(Box<Document>, usize),
    Imported(Vec<[u8; 4]>, u32, u32, PathBuf),
    Saved(PathBuf),
    RasterSaved(PathBuf, Box<Document>),
    Exported(PathBuf),
    Transformed(Box<Document>, String),
    Selection(Selection),
    Edited(Box<Document>, String, bool),
    History(Box<Document>, usize, bool),
    WorkspaceHistory(Vec<(u64, Document)>),
}

pub(super) struct FileJob {
    receiver: Receiver<Result<Outcome, String>>,
    operation: &'static str,
    pub(super) cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl Editor {
    pub(super) fn store_history_async(&mut self) {
        self.store_workspace_history_async(workspace::HISTORY_RAM_BUDGET, true);
    }

    pub(super) fn store_workspace_history_async(&mut self, budget: usize, retry: bool) {
        if self.file_job.is_some() || self.document.editing() || self.selection_transform.is_some()
        {
            return;
        }
        self.history_storage_paused = false;
        let share = budget / (self.documents.len() + 1);
        let mut documents = self.history_storage_inputs(budget, retry);
        if documents.is_empty() {
            return;
        }
        self.start_cancellable_file_job("store undo history", move |cancel| {
            for (_, document) in &mut documents {
                if !document.store_history_with_budget_cancellable(share, cancel) {
                    return Err(files::CANCELLED.into());
                }
            }
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(files::CANCELLED.into());
            }
            Ok(Outcome::WorkspaceHistory(documents))
        });
    }

    pub(super) fn navigate_history_async(&mut self, steps: usize, redo: bool) {
        if self.file_job.is_some() {
            return;
        }
        let mut document = self.document.clone();
        document.defer_history_storage(true);
        self.start_cancellable_file_job("restore undo history", move |cancel| {
            let mut count = 0;
            for _ in 0..steps {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return Err(files::CANCELLED.into());
                }
                let changed = if redo {
                    document.redo_cancellable(cancel)
                } else {
                    document.undo_cancellable(cancel)
                };
                if !changed {
                    break;
                }
                count += 1;
            }
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(files::CANCELLED.into());
            }
            // A failed read never publishes a partial multi-step navigation.
            if let Some(error) = document.history_error() {
                return Err(error.into());
            }
            Ok(Outcome::History(Box::new(document), count, redo))
        });
    }

    pub(super) fn start_document_edit(&mut self, edit: DocumentEdit) {
        self.finish_stroke();
        let mut document = self.document.snapshot();
        document.set_selection(self.document.selection());
        let label = match edit {
            DocumentEdit::Fill(_) => "Fill selection",
            DocumentEdit::OrientLayer(operation) => match operation {
                Orientation::FlipHorizontal => "Flip layer horizontal",
                Orientation::FlipVertical => "Flip layer vertical",
                _ => "Rotate layer 180 degrees",
            },
            DocumentEdit::Flatten => "Flatten image",
            DocumentEdit::MergeDown => "Merge layer down",
            DocumentEdit::Invert => "Invert colors",
            DocumentEdit::Delete => "Delete pixels",
        };
        self.start_cancellable_file_job(label, move |cancel| {
            let completed = match edit {
                DocumentEdit::Fill(color) => {
                    document.fill_selection_cancellable(color, cancel)?;
                    true
                }
                DocumentEdit::OrientLayer(operation) => {
                    document.reorient_active_cancellable(operation, cancel)?;
                    true
                }
                DocumentEdit::Flatten => document.flatten_cancellable(cancel).is_some(),
                DocumentEdit::MergeDown => document.merge_down_cancellable(cancel).is_some(),
                DocumentEdit::Invert => document.invert_cancellable(cancel),
                DocumentEdit::Delete => document.clear_pixels_cancellable(cancel),
            };
            if !completed {
                return Err(files::CANCELLED.into());
            }
            Ok(Outcome::Edited(Box::new(document), label.into(), false))
        });
    }

    pub(super) fn copy_selection_geometry(&mut self) {
        self.finish_stroke();
        let (width, height) = (self.document.width, self.document.height);
        let selection = self.document.selection();
        let clipboard = self.clipboard.clone();
        self.start_cancellable_file_job("copy selection geometry", move |cancel| {
            let text = paint_rust::selection::clipboard::encode(
                width,
                height,
                selection.as_ref(),
                cancel,
            )?;
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(files::CANCELLED.into());
            }
            crate::clipboard::with_backend(&clipboard, |backend| backend.write_text(&text))?;
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(files::CANCELLED.into());
            }
            Ok(Outcome::SelectionCopied)
        });
    }
    pub(super) fn paste_selection_geometry(&mut self, mode: CombineMode) {
        self.finish_stroke();
        let (width, height) = (self.document.width, self.document.height);
        let previous = self.document.selection();
        let clipboard = self.clipboard.clone();
        self.start_cancellable_file_job("paste selection geometry", move |cancel| {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(files::CANCELLED.into());
            }
            let text = crate::clipboard::with_backend(&clipboard, |backend| backend.read_text())?;
            let candidate = paint_rust::selection::clipboard::decode(width, height, &text, cancel)?;
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

    pub(super) fn invert_selection(&mut self) {
        self.finish_stroke();
        let (width, height) = (self.document.width, self.document.height);
        let previous = self.document.selection().unwrap_or(Selection {
            x: 0,
            y: 0,
            width,
            height,
            mask: None,
            ellipse: None,
        });
        self.start_cancellable_file_job("invert selection", move |cancel| {
            paint_rust::selection::combine_cancellable(
                width,
                height,
                None,
                &previous,
                CombineMode::Subtract,
                cancel,
            )
            .map(Outcome::Selection)
        });
    }

    pub(super) fn start_color_selection(&mut self, x: u32, y: u32) {
        if x >= self.document.width || y >= self.document.height {
            return;
        }
        let document = self.document.snapshot();
        let previous = self.document.selection();
        let mode = self.selection_mode;
        let tolerance = self.color_tolerance;
        let contiguous = self.color_contiguous;
        let merged = self.color_sample_merged;
        self.start_cancellable_file_job("select color", move |cancel| {
            let composite = if merged {
                Some(
                    document
                        .composite_cancellable(cancel)
                        .ok_or(files::CANCELLED)?,
                )
            } else {
                None
            };
            let pixels = composite.as_deref().unwrap_or_else(|| document.pixels());
            let candidate = paint_rust::selection::color_cancellable(
                document.width,
                document.height,
                pixels,
                (x, y),
                tolerance,
                contiguous,
                cancel,
            )?;
            paint_rust::selection::combine_cancellable(
                document.width,
                document.height,
                previous.as_ref(),
                &candidate,
                mode,
                cancel,
            )
            .map(Outcome::Selection)
        });
    }

    pub(super) fn start_fill(&mut self, x: i32, y: i32, color: [u8; 4]) {
        let mut document = self.document.snapshot();
        document.set_selection(self.document.selection());
        let options = paint_rust::selection::ColorMatch {
            tolerance: self.color_tolerance,
            contiguous: self.color_contiguous,
        };
        let merged = self.color_sample_merged;
        self.start_cancellable_file_job("fill image", move |cancel| {
            document.begin("Flood fill");
            document.fill_sampled_cancellable((x, y), color, options, merged, cancel)?;
            document.commit();
            Ok(Outcome::Edited(
                Box::new(document),
                "Flood fill".into(),
                false,
            ))
        });
    }

    pub(super) fn repeat_effect(&mut self) {
        let Some(effect) = self.last_effect else {
            return;
        };
        if self.file_job.is_some() || self.text_dialog.is_some() || self.effect_dialog.is_some() {
            return;
        }
        self.finish_stroke();
        let mut document = self.document.snapshot();
        document.set_selection(self.document.selection());
        self.start_cancellable_file_job("repeat effect", move |cancel| {
            let pixels = paint_rust::effects::render(
                document.pixels(),
                document.width,
                document.height,
                document.selection(),
                effect,
                cancel,
            )
            .ok_or(files::CANCELLED)?;
            document.begin(effect.name());
            document.publish_preview(pixels)?;
            document.commit();
            Ok(Outcome::Edited(
                Box::new(document),
                effect.name().into(),
                false,
            ))
        });
    }

    pub(super) fn discover_fonts(&mut self) {
        self.start_cancellable_file_job("find installed fonts", |cancel| {
            Ok(Outcome::FontCatalog(
                paint_rust::fonts::installed_cancellable(cancel)?,
            ))
        });
    }
    pub(super) fn load_installed_font(&mut self, record: paint_rust::fonts::InstalledFont) {
        if self.text_dialog.is_none() {
            return;
        }
        self.start_cancellable_file_job("load installed font", move |cancel| {
            let font =
                paint_rust::text::load_font_face_cancellable(&record.path, record.index, cancel)?;
            Ok(Outcome::TextFont(font, record.label()))
        });
    }

    pub(super) fn load_text_font(&mut self, path: PathBuf) {
        if self.text_dialog.is_none() {
            return;
        }
        self.start_cancellable_file_job("load font", move |cancel| {
            let font = paint_rust::text::load_font_cancellable(&path, cancel)?;
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            Ok(Outcome::TextFont(font, name))
        });
    }

    pub(super) fn start_file_job(
        &mut self,
        operation: &'static str,
        work: impl FnOnce() -> Result<Outcome, String> + Send + 'static,
    ) -> bool {
        if self.file_job.is_some() {
            return false;
        }
        let (sender, receiver) = mpsc::channel();
        let ctx = self.context.clone();
        self.file_job = Some(FileJob {
            receiver,
            operation,
            cancel: None,
        });
        self.status = format!("Working: {operation}…");
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|_| Err("The file worker stopped unexpectedly".into()));
            let _ = sender.send(result);
            ctx.request_repaint();
        });
        true
    }

    pub(super) fn start_cancellable_file_job(
        &mut self,
        operation: &'static str,
        work: impl FnOnce(&std::sync::atomic::AtomicBool) -> Result<Outcome, String> + Send + 'static,
    ) -> bool {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        if !self.start_file_job(operation, move || {
            let result = work(&worker_cancel);
            if result.is_err() && worker_cancel.load(Ordering::Relaxed) {
                Err(files::CANCELLED.into())
            } else {
                result
            }
        }) {
            return false;
        }
        self.file_job.as_mut().expect("job started").cancel = Some(cancel);
        true
    }

    pub(super) fn poll_file_job(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.file_job else { return };
        let result = match job.receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("The file worker stopped unexpectedly".into()),
        };
        let operation = job.operation;
        self.file_job = None;
        match result {
            Ok(Outcome::SelectionCopied) => {
                self.status = "Copied selection geometry as JSON text".into()
            }
            Ok(Outcome::FontCatalog(fonts)) => {
                self.status = format!("Found {} installed font faces", fonts.len());
                self.system_fonts = fonts;
            }
            Ok(Outcome::TextFont(font, name)) => {
                if let Some(dialog) = &mut self.text_dialog {
                    self.text_font = Some((font.clone(), name.clone()));
                    dialog.font = font;
                    dialog.font_name = name;
                    dialog.preview.request_text(crate::preview::TextInput {
                        layout: dialog.layout,
                        font: dialog.font.clone(),
                        content: dialog.content.clone(),
                        origin: dialog.origin,
                        size: dialog.size,
                        color: dialog.color.to_srgba_unmultiplied(),
                    });
                    self.status = "Font loaded".into();
                }
            }
            Ok(Outcome::WorkspaceHistory(documents)) => self.publish_history_storage(documents),
            Ok(Outcome::History(mut document, count, redo)) => {
                document.defer_history_storage(true);
                self.document = *document;
                self.texture_dirty |= count > 0;
                self.status = self
                    .document
                    .history_error()
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        if count == 0 {
                            "Undo history stored".into()
                        } else {
                            format!(
                                "{} {count} operation(s)",
                                if redo { "Redone" } else { "Undone" }
                            )
                        }
                    });
            }
            Ok(Outcome::PastedImage(document)) => {
                self.install_document(*document, None);
                self.status = "Pasted clipboard into a new unsaved image".into();
            }
            Ok(Outcome::Opened(document, path)) => {
                self.preferences.remember(&path);
                self.install_document(*document, Some(path));
                self.status = "Image opened".into();
            }
            Ok(Outcome::Recovered(mut document, index)) => {
                document.mark_unsaved();
                self.install_document(*document, None);
                let candidate = self.recoverable.remove(index);
                if let Some(recovery) = &mut self.recovery {
                    recovery.adopt(candidate);
                }
                self.status = "Recovered unsaved work — save it as a project".into();
                self.recovery_prompt = false;
            }
            Ok(Outcome::Imported(pixels, width, height, path)) => {
                let name = path.file_stem().unwrap_or_default().to_string_lossy();
                match self.document.import_layer(width, height, &pixels, &name) {
                    Ok(_) => {
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
            Ok(Outcome::Transformed(document, label)) => {
                match self.document.apply_transformed(*document, &label) {
                    Ok(changed) => {
                        self.texture_dirty |= changed;
                        self.status = if changed {
                            format!("{label} completed")
                        } else {
                            "Image unchanged".into()
                        };
                    }
                    Err(error) => self.status = format!("Could not {operation}: {error}"),
                }
            }
            Ok(Outcome::Copied(document)) => {
                if let Some(document) = document {
                    let selection = self.document.selection();
                    match self.document.apply_transformed(*document, "Cut pixels") {
                        Ok(changed) => {
                            self.document.set_selection(selection);
                            self.texture_dirty |= changed;
                            self.status = "Cut pixels to clipboard".into();
                        }
                        Err(error) => self.status = format!("Could not cut image: {error}"),
                    }
                } else {
                    self.status = "Copied image to clipboard".into();
                }
            }
            Ok(Outcome::Edited(document, label, moved)) => {
                let selection = if moved {
                    document.selection()
                } else {
                    self.document.selection()
                };
                let selection_changed = selection != self.document.selection();
                match self.document.apply_transformed(*document, &label) {
                    Ok(changed) => {
                        if !changed && selection_changed {
                            self.document.set_selection_with_history(selection, &label);
                        } else {
                            self.document.set_selection(selection);
                        }
                        self.texture_dirty |= changed;
                        self.status = if changed || selection_changed {
                            format!("{label} completed")
                        } else {
                            "Image unchanged".into()
                        };
                    }
                    Err(error) => self.status = format!("Could not {operation}: {error}"),
                }
            }
            Ok(Outcome::Selection(selection)) => {
                let empty = selection.width == 0 || selection.height == 0;
                self.document
                    .set_selection_with_history(Some(selection), operation);
                self.status = if empty {
                    "Selection is empty".into()
                } else {
                    "Selection updated".into()
                };
            }
            Ok(Outcome::RasterSaved(path, document)) => {
                match self
                    .document
                    .apply_transformed(*document, "Save flattened raster")
                {
                    Ok(_) => {
                        self.texture_dirty = true;
                        self.complete_save(path, ctx);
                    }
                    Err(error) => {
                        self.status = format!(
                            "Image was written, but the document could not be updated: {error}"
                        );
                        self.pending = self
                            .after_save
                            .take()
                            .filter(|action| !matches!(action, Action::SaveAll));
                    }
                }
            }
            Ok(Outcome::Saved(path)) => self.complete_save(path, ctx),
            Ok(Outcome::Exported(path)) => {
                self.status = format!(
                    "Exported {} — save the project to preserve editable layers",
                    path.display()
                );
            }
            Err(error) => {
                if operation == "open image" && error != files::CANCELLED {
                    self.open_errors.push(error.clone());
                }
                if error == files::CANCELLED && operation == "store undo history" {
                    self.history_storage_paused = true;
                }
                self.status = if self.history_storage_paused && operation == "store undo history" {
                    "Undo storage paused; history remains in RAM. Resume in History.".into()
                } else if error == files::CANCELLED {
                    format!("Cancelled: {operation}")
                } else {
                    format!("Could not {operation}: {error}")
                };
                // A failed save must not proceed to New/Open/Close.
                if let Some(action) = self.after_save.take()
                    && !matches!(action, Action::SaveAll)
                {
                    self.pending = Some(action);
                }
            }
        }
    }

    pub(super) fn transform_in_background(
        &mut self,
        operation: &'static str,
        transform: impl FnOnce(&mut Document, &std::sync::atomic::AtomicBool) -> Result<bool, String>
        + Send
        + 'static,
    ) -> bool {
        self.finish_stroke();
        let mut snapshot = self.document.snapshot();
        snapshot.set_selection(self.document.selection());
        self.start_cancellable_file_job(operation, move |cancel| {
            transform(&mut snapshot, cancel)?;
            let label = snapshot
                .history()
                .next()
                .unwrap_or("Transform image")
                .to_owned();
            Ok(Outcome::Transformed(Box::new(snapshot.snapshot()), label))
        })
    }

    fn complete_save(&mut self, path: PathBuf, ctx: &egui::Context) {
        self.preferences.remember(&path);
        self.document.mark_saved();
        self.status = format!("Saved {}", path.display());
        self.path = Some(path);
        self.clear_recovery();
        if let Some(action) = self.after_save.take() {
            self.perform(action, ctx);
        }
    }
    pub(super) fn save_raster_encoded(
        &mut self,
        path: PathBuf,
        format: files::RasterFormat,
        bytes: std::sync::Arc<Vec<u8>>,
    ) -> bool {
        self.start_cancellable_file_job("save raster image", move |cancel| {
            let document = files::decode_raster_bytes(&bytes, format, cancel)?;
            files::save_encoded_raster(&bytes, &path, cancel)?;
            Ok(Outcome::RasterSaved(path, Box::new(document)))
        })
    }

    pub(super) fn save_to_path(&mut self, path: PathBuf) -> bool {
        if self.file_job.is_some() || self.export_dialog.is_some() {
            return false;
        }
        if let Some(format) = files::RasterFormat::from_path(&path) {
            self.begin_raster_save(path, format);
            return true;
        }
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("prust") || e.eq_ignore_ascii_case("pdn"))
        {
            self.status =
                "Could not save: choose PRUST, PDN or a supported raster extension.".into();
            return false;
        }
        let snapshot = self.document.snapshot();
        self.start_cancellable_file_job("save project", move |cancel| {
            files::save_project_cancellable(&snapshot, &path, cancel)?;
            Ok(Outcome::Saved(path))
        })
    }
}
