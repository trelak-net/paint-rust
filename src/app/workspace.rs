use super::*;

pub(super) struct DocumentSlot {
    id: u64,
    document: Document,
    path: Option<PathBuf>,
    zoom: f32,
    offset: Vec2,
    texture: Option<TextureHandle>,
    renderer: crate::canvas_renderer::CanvasRenderer,
    history_paused: bool,
    recovery: Option<paint_rust::recovery::Recovery>,
}
impl Editor {
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
            history_paused: std::mem::replace(&mut self.history_storage_paused, false),
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
            || self.effect_dialog.is_some()
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
        std::mem::swap(&mut self.history_storage_paused, &mut slot.history_paused);
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
