use super::*;
use paint_rust::selection::MaskTransform;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
};
struct Request {
    generation: u64,
    parameters: MaskTransform,
    cancel: Arc<AtomicBool>,
}
pub(super) struct Dialog {
    original: Selection,
    parameters: MaskTransform,
    locked: bool,
    requests: Sender<Request>,
    results: Receiver<(u64, Result<Selection, String>)>,
    cancel: Arc<AtomicBool>,
    generation: u64,
    ready: bool,
    error: Option<String>,
}
impl Dialog {
    fn new(ctx: egui::Context, original: Selection, width: u32, height: u32) -> Self {
        let (requests, receive) = mpsc::channel::<Request>();
        let (send, results) = mpsc::channel();
        let source = original.clone();
        std::thread::spawn(move || {
            while let Ok(mut request) = receive.recv() {
                while let Ok(newest) = receive.try_recv() {
                    request = newest;
                }
                let result = paint_rust::selection::transform_cancellable(
                    width,
                    height,
                    &source,
                    request.parameters,
                    &request.cancel,
                );
                if !request.cancel.load(Ordering::Relaxed) {
                    if send.send((request.generation, result)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            }
        });
        Self {
            original,
            parameters: Default::default(),
            locked: true,
            requests,
            results,
            cancel: Arc::new(AtomicBool::new(false)),
            generation: 0,
            ready: true,
            error: None,
        }
    }
    fn request(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.generation += 1;
        self.ready = false;
        self.error = None;
        if self
            .requests
            .send(Request {
                generation: self.generation,
                parameters: self.parameters,
                cancel: self.cancel.clone(),
            })
            .is_err()
        {
            self.error = Some("Selection preview worker stopped".into());
        }
    }
    fn poll(&mut self) -> Option<Selection> {
        let mut latest = None;
        loop {
            let (generation, result) = match self.results.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.error = Some("Selection preview worker stopped".into());
                    break;
                }
            };
            if generation == self.generation {
                match result {
                    Ok(selection) => {
                        latest = Some(selection);
                        self.ready = true;
                    }
                    Err(error) => self.error = Some(error),
                }
            }
        }
        latest
    }
}
impl Drop for Dialog {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Editor {
    pub(super) fn begin_selection_transform(&mut self, ctx: &egui::Context) {
        if self.file_job.is_some()
            || self.effect_dialog.is_some()
            || self.text_dialog.is_some()
            || self.selection_transform.is_some()
        {
            return;
        }
        self.finish_stroke();
        if self.file_job.is_some() {
            return;
        }
        if let Some(original) = self.document.selection() {
            self.selection_transform = Some(Dialog::new(
                ctx.clone(),
                original,
                self.document.width,
                self.document.height,
            ));
        }
    }
    pub(super) fn cancel_selection_transform(&mut self) {
        if let Some(dialog) = self.selection_transform.take() {
            self.document.set_selection(Some(dialog.original.clone()));
        }
    }
    pub(super) fn show_selection_transform(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.selection_transform.take() else {
            return;
        };
        if let Some(selection) = dialog.poll() {
            self.document.set_selection(Some(selection));
        }
        let mut open = true;
        let mut apply = false;
        let mut cancel = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
        egui::Window::new("Transform selection boundary")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .vscroll(true)
            .max_height((ctx.content_rect().height() - 80.0).max(180.0))
            .show(ctx, |ui| {
                let mut changed = false;
                ui.checkbox(&mut dialog.locked, "Keep proportions");
                for axis in 0..2 {
                    if ui
                        .add(
                            egui::DragValue::new(&mut dialog.parameters.scale[axis])
                                .range(0.01..=64.0)
                                .speed(0.01)
                                .prefix(if axis == 0 { "Scale X " } else { "Scale Y " }),
                        )
                        .changed()
                    {
                        if dialog.locked {
                            dialog.parameters.scale[1 - axis] = dialog.parameters.scale[axis];
                        }
                        changed = true;
                    }
                }
                changed |= ui
                    .add(
                        egui::Slider::new(&mut dialog.parameters.angle, -180.0..=180.0)
                            .text("Angle (degrees)"),
                    )
                    .changed();
                for axis in 0..2 {
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut dialog.parameters.offset[axis])
                                .range(-1e6..=1e6)
                                .speed(1.0)
                                .prefix(if axis == 0 { "Offset X " } else { "Offset Y " })
                                .suffix(" px"),
                        )
                        .changed();
                }
                changed |= ui
                    .checkbox(&mut dialog.parameters.flip[0], "Flip horizontal")
                    .changed();
                changed |= ui
                    .checkbox(&mut dialog.parameters.flip[1], "Flip vertical")
                    .changed();
                if ui.button("Reset").clicked() {
                    dialog.parameters = Default::default();
                    changed = true;
                }
                if changed {
                    dialog.request();
                }
                ui.small(
                    "Changes only the selection boundary. Pixels and image history stay unchanged.",
                );
                if let Some(error) = &dialog.error {
                    ui.label(error);
                } else if !dialog.ready {
                    ui.spinner();
                    ui.label("Updating selection…");
                }
                ui.horizontal(|ui| {
                    apply = ui
                        .add_enabled(dialog.ready, egui::Button::new("Apply"))
                        .clicked();
                    cancel |= ui.button("Cancel").clicked();
                });
            });
        apply |= dialog.ready
            && !ctx.wants_keyboard_input()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
        if !open || cancel {
            self.document.set_selection(Some(dialog.original.clone()));
            self.status = "Selection transform cancelled".into();
        } else if apply {
            self.status = "Selection boundary transformed".into();
        } else {
            self.selection_transform = Some(dialog);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wait(editor: &mut Editor, ctx: &egui::Context) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !editor.selection_transform.as_ref().unwrap().ready {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                    ..Default::default()
                },
                |ctx| editor.show_selection_transform(ctx),
            );
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    #[test]
    fn preview_cancel_preserves_pixels_history_path_and_document_identity() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.install_document(
            Document::new(20, 20, [100, 120, 140, 255]).unwrap(),
            Some("first.prust".into()),
        );
        editor.document.invert();
        editor.install_document(
            Document::new(20, 20, [10, 20, 30, 255]).unwrap(),
            Some("second.prust".into()),
        );
        editor.document.invert();
        let original = Selection::elliptical(2, 2, 8, 8);
        editor.document.set_selection(Some(original.clone()));
        let pixels = editor.document.shared_pixels();
        let active = editor.active_document_id;
        let path = editor.path.clone();
        let history = editor
            .document
            .history()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        editor.begin_selection_transform(&ctx);
        let dialog = editor.selection_transform.as_mut().unwrap();
        dialog.parameters.offset = [5.0, 3.0];
        dialog.parameters.angle = 45.0;
        dialog.request();
        editor.activate_document(0);
        assert_eq!(editor.active_document_id, active);
        editor.undo();
        assert_eq!(editor.document.history().count(), history.len());
        editor.store_history_async();
        assert!(editor.file_job.is_none());
        wait(&mut editor, &ctx);
        assert_ne!(editor.document.selection(), Some(original.clone()));
        assert_eq!(editor.document.pixels(), pixels.as_slice());
        assert_eq!(editor.path, path);
        assert_eq!(
            editor.document.history().collect::<Vec<_>>(),
            history.iter().map(String::as_str).collect::<Vec<_>>()
        );
        editor.cancel_selection_transform();
        assert_eq!(editor.document.selection(), Some(original));
        assert!(editor.document.dirty());
        assert_eq!(editor.document.pixels(), pixels.as_slice());
    }
    #[test]
    fn only_latest_preview_generation_can_publish_and_escape_restores_original() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(100, 100, [20, 30, 40, 255]).unwrap();
        let original = Selection::elliptical(10, 10, 40, 40);
        editor.document.set_selection(Some(original.clone()));
        editor.begin_selection_transform(&ctx);
        let dialog = editor.selection_transform.as_mut().unwrap();
        for i in 0..30 {
            dialog.parameters.angle = i as f32;
            dialog.parameters.scale = [2.0; 2];
            dialog.request();
        }
        dialog.parameters = Default::default();
        dialog.request();
        wait(&mut editor, &ctx);
        assert_eq!(editor.document.selection(), Some(original.clone()));
        let dialog = editor.selection_transform.as_mut().unwrap();
        dialog.parameters.offset = [20.0, 20.0];
        dialog.request();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                events: vec![egui::Event::Key {
                    key: Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| editor.show_selection_transform(ctx),
        );
        assert!(editor.selection_transform.is_none());
        assert_eq!(editor.document.selection(), Some(original));
        assert!(!editor.document.dirty());
        assert!(!editor.document.can_undo());
    }
    #[test]
    fn enter_applies_latest_mask_without_creating_an_image_edit() {
        let ctx = egui::Context::default();
        let mut editor = Editor::blank(&ctx);
        editor.document = Document::new(20, 20, [20, 30, 40, 255]).unwrap();
        let original = Selection::elliptical(2, 2, 6, 6);
        editor.document.set_selection(Some(original));
        editor.begin_selection_transform(&ctx);
        let dialog = editor.selection_transform.as_mut().unwrap();
        dialog.parameters.offset = [5.0, 3.0];
        dialog.request();
        wait(&mut editor, &ctx);
        let result = editor.document.selection();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                events: vec![egui::Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| editor.show_selection_transform(ctx),
        );
        assert!(editor.selection_transform.is_none());
        assert_eq!(editor.document.selection(), result);
        assert_eq!(editor.document.pixels(), &[[20, 30, 40, 255]; 400]);
        assert!(!editor.document.dirty());
        assert!(!editor.document.can_undo());
    }
}
