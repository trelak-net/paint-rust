use super::*;
use std::sync::mpsc::{self, Receiver, TryRecvError};

pub(super) enum Outcome {
    Opened(Box<Document>, PathBuf),
    Recovered(Box<Document>, usize),
    Imported(Vec<[u8; 4]>, u32, u32, PathBuf),
    Saved(PathBuf),
    Exported(PathBuf),
    Transformed(Box<Document>, String),
}

pub(super) struct FileJob {
    receiver: Receiver<Result<Outcome, String>>,
    operation: &'static str,
    pub(super) cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl Editor {
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
            Ok(Outcome::Opened(document, path)) => {
                self.document = *document;
                self.path = Some(path);
                self.texture_dirty = true;
                self.last_point = None;
                self.zoom = 1.0;
                self.canvas_offset = Vec2::ZERO;
                self.status = "Image opened".into();
                self.clear_recovery();
            }
            Ok(Outcome::Recovered(mut document, index)) => {
                document.mark_unsaved();
                self.clear_recovery();
                self.document = *document;
                self.path = None;
                self.texture_dirty = true;
                self.zoom = 1.0;
                self.canvas_offset = Vec2::ZERO;
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
            Ok(Outcome::Saved(path)) => {
                self.document.mark_saved();
                self.status = format!("Saved {}", path.display());
                self.path = Some(path);
                self.clear_recovery();
                if let Some(action) = self.after_save.take() {
                    self.perform(action, ctx);
                }
            }
            Ok(Outcome::Exported(path)) => {
                self.status = format!(
                    "Exported {} — save the project to preserve editable layers",
                    path.display()
                );
            }
            Err(error) => {
                self.status = if error == files::CANCELLED {
                    format!("Cancelled: {operation}")
                } else {
                    format!("Could not {operation}: {error}")
                };
                // A failed save must not proceed to New/Open/Close.
                if let Some(action) = self.after_save.take() {
                    self.pending = Some(action);
                }
            }
        }
    }

    pub(super) fn transform_in_background(
        &mut self,
        operation: &'static str,
        transform: impl FnOnce(&mut Document) -> Result<bool, String> + Send + 'static,
    ) -> bool {
        self.finish_stroke();
        let mut snapshot = self.document.snapshot();
        self.start_file_job(operation, move || {
            transform(&mut snapshot)?;
            let label = snapshot
                .history()
                .next()
                .unwrap_or("Transform image")
                .to_owned();
            Ok(Outcome::Transformed(Box::new(snapshot.snapshot()), label))
        })
    }

    pub(super) fn save_to_path(&mut self, path: PathBuf) -> bool {
        let snapshot = self.document.snapshot();
        self.start_cancellable_file_job("save project", move |cancel| {
            files::save_project_cancellable(&snapshot, &path, cancel)?;
            Ok(Outcome::Saved(path))
        })
    }
}
