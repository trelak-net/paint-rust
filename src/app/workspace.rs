use super::*;

pub(super) const HISTORY_DISK_BUDGET: u64 = 1024 * 1024 * 1024;

pub(super) const HISTORY_RAM_BUDGET: usize = 128 * 1024 * 1024;

pub(super) struct DocumentSlot {
    id: u64,
    document: Document,
    path: Option<PathBuf>,
    zoom: f32,
    offset: Vec2,
    texture: Option<TextureHandle>,
    renderer: crate::canvas_renderer::CanvasRenderer,
    recovery: Option<paint_rust::recovery::Recovery>,
}
impl Editor {
    pub(super) fn workspace_history_disk_bytes(&self) -> u64 {
        self.document.history_disk_bytes()
            + self
                .documents
                .iter()
                .map(|slot| slot.document.history_disk_bytes())
                .sum::<u64>()
    }

    pub(super) fn workspace_history_bytes(&self) -> usize {
        self.document.history_bytes()
            + self
                .documents
                .iter()
                .map(|slot| slot.document.history_bytes())
                .sum::<usize>()
    }

    pub(super) fn workspace_history_error(&self) -> Option<&str> {
        self.document.history_error().or_else(|| {
            self.documents
                .iter()
                .find_map(|slot| slot.document.history_error())
        })
    }

    pub(super) fn workspace_history_needs_storage(&self, budget: usize) -> bool {
        if self.history_storage_paused || self.workspace_history_bytes() <= budget {
            return false;
        }
        let share = budget / (self.documents.len() + 1);
        std::iter::once(&self.document)
            .chain(self.documents.iter().map(|slot| &slot.document))
            .any(|doc| doc.history_error().is_none() && doc.history_bytes() > share)
    }

    pub(super) fn history_storage_inputs(
        &self,
        budget: usize,
        retry: bool,
    ) -> Vec<(u64, Document)> {
        let share = budget / (self.documents.len() + 1);
        std::iter::once((self.active_document_id, &self.document))
            .chain(self.documents.iter().map(|slot| (slot.id, &slot.document)))
            .filter(|(_, doc)| {
                doc.history_bytes() > share && (retry || doc.history_error().is_none())
            })
            .map(|(id, doc)| (id, doc.clone()))
            .collect()
    }

    pub(super) fn publish_history_storage(&mut self, documents: Vec<(u64, Document)>) {
        for (id, mut document) in documents {
            document.defer_history_storage(true);
            if id == self.active_document_id {
                self.document = document;
            } else if let Some(slot) = self.documents.iter_mut().find(|slot| slot.id == id) {
                slot.document = document;
            }
        }
        self.status = self
            .workspace_history_error()
            .map(str::to_owned)
            .unwrap_or_else(|| "Workspace undo history stored".into());
    }

    pub(super) fn document_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_max_width(360.0);
        if ui.button("Next document    Ctrl+Tab").clicked() {
            self.cycle_document(1);
            ui.close();
        }
        if ui.button("Previous document    Ctrl+Shift+Tab").clicked() {
            self.cycle_document(-1);
            ui.close();
        }
        ui.separator();
        let mut activate = None;
        egui::ScrollArea::vertical()
            .max_height((ui.ctx().content_rect().height() - 160.0).clamp(80.0, 400.0))
            .show(ui, |ui| {
                for id in &self.document_order {
                    let (document, path) = if *id == self.active_document_id {
                        (&self.document, &self.path)
                    } else {
                        let Some(slot) = self.documents.iter().find(|slot| slot.id == *id) else {
                            continue;
                        };
                        (&slot.document, &slot.path)
                    };
                    let name = path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|| format!("Untitled {}", id + 1));
                    let label = format!("{name}{}", if document.dirty() { " *" } else { "" });
                    ui.push_id(id, |ui| {
                        if ui
                            .add(
                                egui::Button::new(label)
                                    .selected(*id == self.active_document_id)
                                    .truncate(),
                            )
                            .on_hover_text(
                                path.as_ref()
                                    .map(|p| p.display().to_string())
                                    .unwrap_or(name),
                            )
                            .clicked()
                        {
                            activate = Some(*id);
                            ui.close();
                        }
                    });
                }
            });
        if let Some(id) = activate {
            self.activate_document(id);
        }
    }

    pub(super) fn save_all_next(&mut self) {
        if !self.document.dirty() {
            let next = self
                .document_order
                .iter()
                .find(|id| {
                    self.documents
                        .iter()
                        .any(|slot| slot.id == **id && slot.document.dirty())
                })
                .copied();
            if let Some(id) = next {
                self.activate_document(id);
            } else {
                self.status = "All documents are saved".into();
                return;
            }
        }
        if self.save(false) {
            self.after_save = Some(Action::SaveAll);
        } else {
            self.after_save = None;
            self.status = "Save all stopped; remaining documents are still unsaved".into();
        }
    }

    fn reset_document_view(&mut self) {
        self.texture_dirty = true;
        self.selection_texture = None;
        self.polygon_points.clear();
        self.lasso.cancel();
        self.last_point = None;
        self.shape_origin = None;
        self.selection_before = None;
        self.fit_canvas = false;
        self.panning = false;
        self.zoom_origin = None;
        self.suppress_stroke = false;
    }

    pub(super) fn install_document(&mut self, document: Document, path: Option<PathBuf>) {
        let preserve = !self.initial_placeholder
            || self.document.dirty()
            || self.path.is_some()
            || self.document.can_undo()
            || self.document.can_redo();
        let recovery = if preserve {
            self.recovery.as_ref().and_then(|r| match r.sibling() {
                Ok(recovery) => Some(recovery),
                Err(error) => {
                    self.status = format!("Recovery unavailable for new document: {error}");
                    None
                }
            })
        } else {
            self.clear_recovery();
            self.recovery.take()
        };
        if self.texture_dirty {
            self.canvas_renderer.request(&self.document);
        }
        let previous = DocumentSlot {
            id: self.active_document_id,
            document: std::mem::replace(&mut self.document, document),
            path: std::mem::replace(&mut self.path, path),
            zoom: std::mem::replace(&mut self.zoom, 1.0),
            offset: std::mem::replace(&mut self.canvas_offset, Vec2::ZERO),
            texture: self.texture.take(),
            renderer: std::mem::replace(
                &mut self.canvas_renderer,
                crate::canvas_renderer::CanvasRenderer::new(self.context.clone()),
            ),
            recovery: std::mem::replace(&mut self.recovery, recovery),
        };
        if preserve {
            self.documents.push(previous);
            self.active_document_id = self.next_document_id;
            self.next_document_id += 1;
            self.document_order.push(self.active_document_id);
        }
        self.initial_placeholder = false;
        self.reset_document_view();
    }

    pub(super) fn activate_document(&mut self, id: u64) {
        if id == self.active_document_id
            || self.file_job.is_some()
            || self.document.editing()
            || self.text_dialog.is_some()
            || self.export_dialog.is_some()
            || self.effect_dialog.is_some()
            || self.selection_transform.is_some()
            || self.paste_confirmation.is_some()
        {
            return;
        }
        let Some(index) = self.documents.iter().position(|s| s.id == id) else {
            return;
        };
        if self.texture_dirty {
            self.canvas_renderer.request(&self.document);
        }
        let slot = &mut self.documents[index];
        std::mem::swap(&mut self.document, &mut slot.document);
        std::mem::swap(&mut self.path, &mut slot.path);
        std::mem::swap(&mut self.zoom, &mut slot.zoom);
        std::mem::swap(&mut self.canvas_offset, &mut slot.offset);
        std::mem::swap(&mut self.texture, &mut slot.texture);
        std::mem::swap(&mut self.canvas_renderer, &mut slot.renderer);
        std::mem::swap(&mut self.recovery, &mut slot.recovery);
        std::mem::swap(&mut self.active_document_id, &mut slot.id);
        self.reset_document_view();
    }

    pub(super) fn close_current_document(&mut self) {
        self.clear_recovery();
        let closed = self.active_document_id;
        self.document_order.retain(|id| *id != closed);
        if let Some(next) = self.document_order.last().copied() {
            self.activate_document(next);
            self.documents.retain(|s| s.id != closed);
        } else {
            self.document = Document::new(960, 640, [255; 4]).unwrap();
            self.path = None;
            self.texture = None;
            self.canvas_renderer =
                crate::canvas_renderer::CanvasRenderer::new(self.context.clone());
            self.zoom = 1.0;
            self.canvas_offset = Vec2::ZERO;
            self.history_storage_paused = false;
            self.active_document_id = self.next_document_id;
            self.next_document_id += 1;
            self.document_order.push(self.active_document_id);
            self.initial_placeholder = true;
            self.reset_document_view();
        }
    }

    pub(super) fn cycle_document(&mut self, direction: isize) {
        let index = self
            .document_order
            .iter()
            .position(|id| *id == self.active_document_id)
            .unwrap();
        let next =
            (index as isize + direction).rem_euclid(self.document_order.len() as isize) as usize;
        self.activate_document(self.document_order[next]);
    }

    fn reorder_document(&mut self, source: u64, target: u64) {
        if source == target {
            return;
        }
        let Some(from) = self.document_order.iter().position(|id| *id == source) else {
            return;
        };
        let Some(to) = self.document_order.iter().position(|id| *id == target) else {
            return;
        };
        self.document_order.remove(from);
        self.document_order.insert(to, source);
    }

    pub(super) fn document_tabs(&mut self, ctx: &egui::Context, enabled: bool) {
        egui::TopBottomPanel::top("documents").show(ctx, |ui| {
            ui.spacing_mut().button_padding = egui::vec2(6.0, 2.0);
            if !enabled || self.document.editing() || self.pending.is_some() || self.new_dialog {
                ui.disable();
            }
            let mut activate = None;
            let mut close = None;
            let mut reorder = None;
            egui::ScrollArea::horizontal().id_salt("document-tabs").show(ui, |ui| {
                ui.horizontal(|ui| {
                    for id in &self.document_order {
                        let (doc, path, texture) = if *id == self.active_document_id {
                            (&self.document, &self.path, &self.texture)
                        } else {
                            let slot = self.documents.iter().find(|s| s.id == *id).unwrap();
                            (&slot.document, &slot.path, &slot.texture)
                        };
                        let name = path.as_ref().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| format!("Untitled {}", id + 1));
                        let label = format!("{}{}", name, if doc.dirty() { " *" } else { "" });
                        let response = ui.push_id(id, |ui| {
                            let button = if let Some(texture) = texture {
                                let scale = (48.0 / doc.width as f32).min(32.0 / doc.height as f32);
                                let size = egui::vec2(doc.width as f32, doc.height as f32) * scale;
                                egui::Button::image_and_text(egui::Image::new((texture.id(),size)), &label)
                            } else { egui::Button::new(&label) };
                            ui.add_sized([180.0, 36.0],button.selected(*id == self.active_document_id)
                                .truncate().sense(Sense::click_and_drag()))
                                .on_hover_text(format!("{name} — {} × {} px\nDrag to reorder; middle-click to close",doc.width,doc.height))
                        }).inner;
                        if response.clicked() { activate = Some(*id); }
                        if response.clicked_by(egui::PointerButton::Middle) { close = Some(*id); }
                        response.dnd_set_drag_payload(*id);
                        if let Some(source) = response.dnd_release_payload::<u64>() { reorder = Some((*source,*id)); }
                    }
                    if ui.button("Close").on_hover_text("Close document (Ctrl+W)").clicked() {
                        close = Some(self.active_document_id);
                    }
                    if ui.button("Close all").clicked() { self.request(Action::CloseAll,ctx); }
                });
            });
            if let Some((source,target)) = reorder { self.reorder_document(source,target); }
            if let Some(id) = activate { self.activate_document(id); }
            if let Some(id) = close {
                self.activate_document(id);
                self.request(Action::CloseDocument,ctx);
            }
        });
    }

    pub(super) fn tick_inactive_recovery(&mut self, ctx: &egui::Context) {
        for slot in &mut self.documents {
            if let Some(update) = slot.renderer.poll() {
                ctx.request_repaint();
                match update {
                    crate::canvas_renderer::Update::Full(image) => {
                        if let Some(texture) = &mut slot.texture {
                            texture.set(image, egui::TextureOptions::NEAREST);
                        } else {
                            slot.texture = Some(ctx.load_texture(
                                format!("document-{}", slot.id),
                                image,
                                egui::TextureOptions::NEAREST,
                            ));
                        }
                    }
                    crate::canvas_renderer::Update::Partial { position, image } => {
                        if let Some(texture) = &mut slot.texture {
                            texture.set_partial(position, image, egui::TextureOptions::NEAREST);
                        } else {
                            slot.renderer.invalidate();
                            slot.renderer.request(&slot.document);
                        }
                    }
                    crate::canvas_renderer::Update::Unchanged => {}
                }
            }
            if let Some(recovery) = &mut slot.recovery {
                let _ = recovery.set_context(paint_rust::recovery::Context {
                    version: 1,
                    workspace: recovery.workspace().to_owned(),
                    order: self
                        .document_order
                        .iter()
                        .position(|id| *id == slot.id)
                        .unwrap_or(0),
                    path: slot.path.as_ref().and_then(|p| std::path::absolute(p).ok()),
                    zoom: slot.zoom,
                    offset: [slot.offset.x, slot.offset.y],
                    active: false,
                });
                let wake = ctx.clone();
                if let Err(error) = recovery.tick(&slot.document, move || wake.request_repaint()) {
                    self.status = format!("Could not update inactive document recovery: {error}");
                }
                ctx.request_repaint_after(std::time::Duration::from_secs(1));
            }
        }
    }
    pub(super) fn clear_inactive_recovery(&mut self) {
        for slot in &mut self.documents {
            if let Some(recovery) = &mut slot.recovery {
                let _ = recovery.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn history_workspace(ctx: &egui::Context) -> Editor {
        let mut editor = Editor::blank(ctx);
        for value in [20, 40, 60] {
            let mut document = Document::new(8, 8, [value, value, value, 255]).unwrap();
            document.defer_history_storage(true);
            for _ in 0..5 {
                document.invert();
            }
            editor.install_document(document, Some(format!("{value}.prust").into()));
        }
        editor
    }

    fn finish_job(editor: &mut Editor, ctx: &egui::Context) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while editor.file_job.is_some() {
            editor.poll_file_job(ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    #[test]
    fn workspace_disk_limit_is_shared_and_preserves_every_document_history() {
        let ctx = egui::Context::default();
        let mut editor = history_workspace(&ctx);
        editor.store_workspace_history_with_limits_async(0, 100, false);
        finish_job(&mut editor, &ctx);
        assert!(editor.workspace_history_disk_bytes() <= 100);
        assert!(editor.workspace_history_error().is_some());
        assert!(editor.workspace_history_bytes() > 0);
        for id in editor.document_order.clone() {
            editor.activate_document(id);
            assert_eq!(editor.document.history().count(), 5);
            for _ in 0..5 {
                assert!(editor.document.undo());
            }
            assert!(!editor.document.dirty());
            for _ in 0..5 {
                assert!(editor.document.redo());
            }
        }
        editor.store_workspace_history_with_limits_async(0, 1_000_000, true);
        finish_job(&mut editor, &ctx);
        assert!(editor.workspace_history_error().is_none());
        assert_eq!(editor.workspace_history_bytes(), 0);
    }
    #[test]
    fn workspace_budget_stores_inactive_history_without_losing_undo_or_views() {
        let ctx = egui::Context::default();
        let mut editor = history_workspace(&ctx);
        let budget = 3 * 8 * 8 * 4 * 2;
        assert!(editor.workspace_history_needs_storage(budget));
        assert_eq!(editor.documents.len(), 2);
        editor.zoom = 3.0;
        let bytes = editor.workspace_history_bytes();
        let active = editor.active_document_id;
        editor.store_workspace_history_async(budget, false);
        // Worker results must be published as a complete batch by the UI thread.
        assert_eq!(editor.workspace_history_bytes(), bytes);
        editor.activate_document(0);
        assert_eq!(editor.active_document_id, active);
        finish_job(&mut editor, &ctx);
        assert!(editor.workspace_history_bytes() <= budget);
        assert!(!editor.workspace_history_needs_storage(budget));
        assert_eq!(editor.zoom, 3.0);
        for id in editor.document_order.clone() {
            editor.activate_document(id);
            let value = (id as u8 + 1) * 20;
            assert_eq!(editor.path, Some(format!("{value}.prust").into()));
            assert!(editor.document.history_disk_bytes() > 0);
            assert_eq!(editor.document.history().count(), 5);
            for _ in 0..5 {
                assert!(editor.document.undo());
            }
            assert_eq!(editor.document.pixels(), &[[value, value, value, 255]; 64]);
            assert!(!editor.document.dirty());
            for _ in 0..5 {
                assert!(editor.document.redo());
            }
            assert_eq!(
                editor.document.pixels(),
                &[[255 - value, 255 - value, 255 - value, 255]; 64]
            );
        }
    }

    #[test]
    fn cancelled_workspace_storage_retains_every_document_and_pause_survives_switching() {
        let ctx = egui::Context::default();
        let mut editor = history_workspace(&ctx);
        let before = editor.workspace_history_bytes();
        let mut documents = editor.history_storage_inputs(0, false);
        let (release, wait) = std::sync::mpsc::channel();
        editor.start_cancellable_file_job("store undo history", move |cancel| {
            // Complete one private clone before cancellation; none may be published.
            assert!(documents[0].1.store_history_with_budget_cancellable(
                0,
                &std::sync::atomic::AtomicBool::new(false)
            ));
            wait.recv().unwrap();
            for (_, document) in &mut documents {
                if !document.store_history_with_budget_cancellable(0, cancel) {
                    return Err(files::CANCELLED.into());
                }
            }
            Ok(file_operations::Outcome::WorkspaceHistory(documents))
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
        finish_job(&mut editor, &ctx);
        assert_eq!(editor.workspace_history_bytes(), before);
        assert!(editor.history_storage_paused);
        for id in editor.document_order.clone() {
            editor.activate_document(id);
            assert!(editor.history_storage_paused);
            assert!(!editor.workspace_history_needs_storage(0));
            assert_eq!(editor.document.history().count(), 5);
            assert_eq!(editor.document.history_disk_bytes(), 0);
        }
        editor.install_document(Document::new(8, 8, [255; 4]).unwrap(), None);
        assert!(editor.history_storage_paused);
        editor.store_workspace_history_async(0, true);
        finish_job(&mut editor, &ctx);
        assert!(!editor.history_storage_paused);
        assert_eq!(editor.workspace_history_bytes(), 0);
    }

    #[test]
    fn switching_restores_history_selection_view_and_paths() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.install_document(
            Document::new(8, 8, [255; 4]).unwrap(),
            Some("first.pdn".into()),
        );
        let first = editor.active_document_id;
        editor.document.invert();
        editor
            .document
            .set_selection(Some(Selection::elliptical(1, 1, 3, 3)));
        let selection = editor.document.selection();
        editor.zoom = 2.0;
        editor.canvas_offset = Vec2::new(12.0, 20.0);
        editor.install_document(Document::new(16, 4, [40; 4]).unwrap(), None);
        let second = editor.active_document_id;
        assert_eq!(editor.documents.len(), 1);
        assert!(!editor.document.dirty());
        editor.document.invert();
        editor.activate_document(first);
        assert_eq!(editor.document.width, 8);
        assert_eq!(editor.document.selection(), selection);
        assert_eq!(editor.zoom, 2.0);
        assert_eq!(editor.canvas_offset, Vec2::new(12.0, 20.0));
        assert_eq!(editor.path, Some("first.pdn".into()));
        editor.document.undo();
        assert!(!editor.document.dirty());
        editor.activate_document(second);
        assert!(editor.document.dirty());
        assert_eq!(editor.document.width, 16);
        assert_eq!(editor.document.history().count(), 1);
    }
    #[test]
    fn close_all_checks_each_dirty_document_and_cancel_keeps_remaining_work() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document.invert();
        let first = editor.active_document_id;
        editor.install_document(Document::new(4, 4, [255; 4]).unwrap(), None);
        editor.document.invert();
        editor.request(Action::CloseAll, &ctx);
        assert!(matches!(editor.pending, Some(Action::CloseAll)));
        assert_eq!(editor.documents.len(), 1);
        editor.pending = None;
        editor.perform(Action::CloseAll, &ctx);
        assert_eq!(editor.active_document_id, first);
        assert!(matches!(editor.pending, Some(Action::CloseAll)));
        assert!(editor.document.dirty());
        // Cancel leaves the unclosed document and its history untouched.
        editor.pending = None;
        assert_eq!(editor.document.history().count(), 1);
        editor.request(Action::CloseAll, &ctx);
        editor.pending = None;
        editor.perform(Action::CloseAll, &ctx);
        assert!(editor.documents.is_empty());
        assert!(!editor.document.dirty());
        assert!(editor.initial_placeholder);
    }
    #[test]
    fn new_does_not_prompt_to_discard_an_existing_document() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document.invert();
        editor.request(Action::New, &ctx);
        assert!(editor.pending.is_none());
        assert!(editor.new_dialog);
        assert!(editor.document.dirty());
    }
    #[test]
    fn background_jobs_prevent_document_switching() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.initial_placeholder = false;
        let first = editor.active_document_id;
        editor.install_document(Document::new(4, 4, [255; 4]).unwrap(), None);
        let second = editor.active_document_id;
        editor.start_fill(0, 0, [0; 4]);
        editor.activate_document(first);
        assert_eq!(editor.active_document_id, second);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while editor.file_job.is_some() {
            editor.poll_file_job(&ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(editor.document.pixels().iter().all(|p| *p == [0; 4]));
        editor.activate_document(first);
        assert!(editor.document.pixels().iter().all(|p| *p == [255; 4]));
    }
    #[test]
    fn reordering_keeps_document_identity_and_keyboard_cycle_follows_tab_order() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.initial_placeholder = false;
        editor.install_document(Document::new(4, 4, [10; 4]).unwrap(), None);
        editor.install_document(Document::new(4, 4, [20; 4]).unwrap(), None);
        editor.reorder_document(2, 0);
        assert_eq!(editor.document_order, vec![2, 0, 1]);
        assert_eq!(editor.active_document_id, 2);
        editor.cycle_document(1);
        assert_eq!(editor.active_document_id, 0);
        editor.cycle_document(-1);
        assert_eq!(editor.active_document_id, 2);
        assert!(editor.document.pixels().iter().all(|p| *p == [20; 4]));
    }

    #[test]
    fn batch_open_continues_after_invalid_file_without_replacing_unsaved_work() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document.invert();
        let original = editor.active_document_id;
        let directory = tempfile::tempdir().unwrap();
        for (name, color) in [("one.prust", [10; 4]), ("two.prust", [20; 4])] {
            files::save_project(
                &Document::new(4, 4, color).unwrap(),
                &directory.path().join(name),
            )
            .unwrap();
        }
        std::fs::write(directory.path().join("bad.pdn"), b"PDN3").unwrap();
        editor
            .open_queue
            .extend(["one.prust", "bad.pdn", "two.prust"].map(|name| directory.path().join(name)));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !editor.open_queue.is_empty() || editor.file_job.is_some() {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1180.0, 800.0))),
                    focused: true,
                    ..Default::default()
                },
                |ctx| editor.show(ctx),
            );
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(editor.documents.len(), 2);
        assert_eq!(editor.open_errors.len(), 1);
        assert!(editor.open_errors[0].contains("bad.pdn"));
        assert!(editor.document.pixels().iter().all(|p| *p == [20; 4]));
        editor.activate_document(original);
        assert!(editor.document.dirty());
        assert_eq!(editor.document.history().count(), 1);
    }

    #[test]
    fn each_document_keeps_an_independent_recovery_session() {
        let ctx = egui::Context::default();
        let root = tempfile::tempdir().unwrap();
        let mut editor = Editor::blank(&ctx);
        editor.recovery = Some(paint_rust::recovery::Recovery::new(root.path()).unwrap());
        editor.document.invert();
        editor.install_document(Document::new(4, 4, [255; 4]).unwrap(), None);
        assert!(editor.recovery.is_some());
        assert!(editor.documents[0].recovery.is_some());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
        editor.close_current_document();
        assert!(editor.document.dirty());
        assert!(editor.recovery.is_some());
    }
}
