use super::*;

pub(super) struct DocumentSlot {
    id: u64,
    document: Document,
    path: Option<PathBuf>,
    zoom: f32,
    offset: Vec2,
    texture: Option<TextureHandle>,
    history_paused: bool,
    recovery: Option<paint_rust::recovery::Recovery>,
}
impl Editor {
    fn reset_document_view(&mut self) {
        self.canvas_renderer = crate::canvas_renderer::CanvasRenderer::new(self.context.clone());
        self.texture_dirty = true;
        self.selection_texture = None;
        self.polygon_points.clear();
        self.last_point = None;
        self.shape_origin = None;
        self.selection_before = None;
        self.fit_canvas = false;
        self.panning = false;
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
        let previous = DocumentSlot {
            id: self.active_document_id,
            document: std::mem::replace(&mut self.document, document),
            path: std::mem::replace(&mut self.path, path),
            zoom: std::mem::replace(&mut self.zoom, 1.0),
            offset: std::mem::replace(&mut self.canvas_offset, Vec2::ZERO),
            texture: self.texture.take(),
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
        let slot = &mut self.documents[index];
        std::mem::swap(&mut self.document, &mut slot.document);
        std::mem::swap(&mut self.path, &mut slot.path);
        std::mem::swap(&mut self.zoom, &mut slot.zoom);
        std::mem::swap(&mut self.canvas_offset, &mut slot.offset);
        std::mem::swap(&mut self.texture, &mut slot.texture);
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

    pub(super) fn document_tabs(&mut self, ctx: &egui::Context, enabled: bool) {
        egui::TopBottomPanel::top("documents").show(ctx, |ui| {
            if !enabled || self.document.editing() {
                ui.disable();
            }
            let mut activate = None;
            egui::ScrollArea::horizontal()
                .id_salt("document-tabs")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for id in &self.document_order {
                            let (doc, path) = if *id == self.active_document_id {
                                (&self.document, &self.path)
                            } else {
                                let slot = self.documents.iter().find(|s| s.id == *id).unwrap();
                                (&slot.document, &slot.path)
                            };
                            let name = path
                                .as_ref()
                                .and_then(|p| p.file_name())
                                .map(|s| s.to_string_lossy().into_owned())
                                .unwrap_or_else(|| format!("Untitled {}", id + 1));
                            let label = format!("{}{}", name, if doc.dirty() { " *" } else { "" });
                            if ui
                                .push_id(id, |ui| {
                                    ui.selectable_label(*id == self.active_document_id, label)
                                        .on_hover_text(format!("{} × {} px", doc.width, doc.height))
                                })
                                .inner
                                .clicked()
                            {
                                activate = Some(*id);
                            }
                        }
                    });
                });
            ui.horizontal(|ui| {
                if ui
                    .button("Close document")
                    .on_hover_text("Ctrl+W")
                    .clicked()
                {
                    self.request(Action::CloseDocument, ctx);
                }
                if ui.button("Close all").clicked() {
                    self.request(Action::CloseAll, ctx);
                }
            });
            if let Some(id) = activate {
                self.activate_document(id);
            }
        });
    }

    pub(super) fn tick_inactive_recovery(&mut self, ctx: &egui::Context) {
        for slot in &mut self.documents {
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
