use super::*;
impl Editor {
    pub(super) fn retain_shape(&mut self) -> bool {
        if !self.keep_shape_editable {
            return false;
        }
        let Some(Effect::Shape(parameters)) =
            self.gesture_preview.as_ref().map(|preview| preview.effect)
        else {
            return false;
        };
        self.shape_edit = Some(parameters);
        self.curve_controls = None;
        self.last_point = None;
        self.shape_origin = None;
        true
    }
    pub(super) fn finish_shape_edit(&mut self, apply: bool) {
        if self.shape_edit.take().is_none() {
            return;
        }
        self.curve_controls = None;
        if apply {
            self.finish_stroke();
        } else {
            self.gesture_preview = None;
            self.document.cancel();
            self.last_point = None;
            self.shape_origin = None;
            self.texture_dirty = true;
            self.status = "Shape cancelled".into();
        }
    }
    fn request_shape_edit_preview(&mut self, parameters: paint_rust::shapes::Parameters) {
        let effect = if let Some(controls) = self
            .curve_controls
            .filter(|_| parameters.shape == Shape::Line)
        {
            Effect::Bezier(paint_rust::bezier::Parameters {
                points: [parameters.from, controls[0], controls[1], parameters.to],
                style: parameters.style,
            })
        } else {
            Effect::Shape(parameters)
        };
        self.request_gesture_preview(effect);
    }
    pub(super) fn shape_handles(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let Some(mut parameters) = self.shape_edit else {
            return;
        };
        let to_screen = |p: (f32, f32)| rect.min + Vec2::new(p.0, p.1) * self.zoom;
        let from = to_screen(parameters.from);
        let to = to_screen(parameters.to);
        let bounds = Rect::from_two_pos(from, to);
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        painter.rect_stroke(
            bounds,
            0.0,
            egui::Stroke::new(1.0_f32, Color32::WHITE),
            egui::StrokeKind::Outside,
        );
        let handles: Vec<(Pos2, i32, i32)> = if let Some(controls) = self.curve_controls {
            let controls = controls.map(to_screen);
            painter.line_segment(
                [from, controls[0]],
                egui::Stroke::new(1.0_f32, Color32::GRAY),
            );
            painter.line_segment([controls[1], to], egui::Stroke::new(1.0_f32, Color32::GRAY));
            vec![
                (from, 0, 0),
                (controls[0], 3, 0),
                (controls[1], 4, 0),
                (to, 2, 2),
                (from.lerp(to, 0.5), 1, 1),
            ]
        } else if parameters.shape == Shape::Line {
            vec![(from, 0, 0), (to, 2, 2), (from.lerp(to, 0.5), 1, 1)]
        } else {
            (0..3)
                .flat_map(|y| {
                    (0..3).map(move |x| {
                        (
                            Pos2::new(
                                from.x + (to.x - from.x) * x as f32 / 2.0,
                                from.y + (to.y - from.y) * y as f32 / 2.0,
                            ),
                            x,
                            y,
                        )
                    })
                })
                .collect()
        };
        let mut changed = false;
        for (index, (position, x, y)) in handles.into_iter().enumerate() {
            let response = ui.interact(
                Rect::from_center_size(position, Vec2::splat(14.0)),
                ui.id().with(("shape-handle", index)),
                Sense::drag(),
            );
            painter.circle_filled(position, 4.0, Color32::WHITE);
            painter.circle_stroke(position, 4.0, egui::Stroke::new(1.5_f32, Color32::BLACK));
            response.clone().on_hover_text(if x >= 3 {
                "Curve control point"
            } else if x == 1 && y == 1 {
                "Move shape"
            } else {
                "Resize shape"
            });
            if response.dragged() {
                let delta = ui.input(|i| i.pointer.delta()) / self.zoom;
                if x >= 3 {
                    if let Some(controls) = &mut self.curve_controls {
                        controls[(x - 3) as usize].0 += delta.x;
                        controls[(x - 3) as usize].1 += delta.y;
                    }
                } else if x == 1 && y == 1 {
                    parameters.from.0 += delta.x;
                    parameters.to.0 += delta.x;
                    parameters.from.1 += delta.y;
                    parameters.to.1 += delta.y;
                    if let Some(controls) = &mut self.curve_controls {
                        for point in controls {
                            point.0 += delta.x;
                            point.1 += delta.y;
                        }
                    }
                } else {
                    if x == 0 {
                        parameters.from.0 += delta.x;
                    }
                    if x == 2 {
                        parameters.to.0 += delta.x;
                    }
                    if y == 0 {
                        parameters.from.1 += delta.y;
                    }
                    if y == 2 {
                        parameters.to.1 += delta.y;
                    }
                }
                changed |= delta != Vec2::ZERO;
            }
        }
        if changed {
            for point in [&mut parameters.from, &mut parameters.to] {
                point.0 = point.0.clamp(-32768.0, 32768.0);
                point.1 = point.1.clamp(-32768.0, 32768.0);
            }
            if let Some(controls) = &mut self.curve_controls {
                for point in controls {
                    point.0 = point.0.clamp(-32768.0, 32768.0);
                    point.1 = point.1.clamp(-32768.0, 32768.0);
                }
            }
            self.shape_edit = Some(parameters);
            self.request_shape_edit_preview(parameters);
        }
    }
    pub(super) fn show_shape_edit(&mut self, ctx: &egui::Context) {
        let Some(mut parameters) = self.shape_edit else {
            return;
        };
        let before = parameters;
        let before_controls = self.curve_controls;
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Edit shape")
            .open(&mut open)
            .collapsible(false)
            .default_pos(Pos2::new(220.0, 200.0))
            .default_width(300.0)
            .max_height((ctx.content_rect().height() - 80.0).max(180.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 200.0).max(100.0))
                    .show(ui, |ui| {
                        ui.label("Drag canvas handles to resize; drag the center handle to move.");
                        egui::ComboBox::from_label("Shape")
                            .selected_text(parameters.shape.label())
                            .show_ui(ui, |ui| {
                                for shape in Shape::ALL {
                                    ui.selectable_value(
                                        &mut parameters.shape,
                                        shape,
                                        shape.label(),
                                    );
                                }
                            });
                        if parameters.shape == Shape::Line {
                            let mut curved = self.curve_controls.is_some();
                            if ui.checkbox(&mut curved, "Cubic curve").changed() {
                                self.curve_controls = curved.then(|| {
                                    [1.0 / 3.0, 2.0 / 3.0].map(|t| {
                                        (
                                            parameters.from.0
                                                + (parameters.to.0 - parameters.from.0) * t,
                                            parameters.from.1
                                                + (parameters.to.1 - parameters.from.1) * t,
                                        )
                                    })
                                });
                            }
                        } else {
                            self.curve_controls = None;
                        }
                        if let Some(controls) = &mut self.curve_controls {
                            for (index, point) in controls.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("Control {}", index + 1));
                                    ui.add(
                                        egui::DragValue::new(&mut point.0)
                                            .range(-32768.0..=32768.0)
                                            .prefix("X "),
                                    );
                                    ui.add(
                                        egui::DragValue::new(&mut point.1)
                                            .range(-32768.0..=32768.0)
                                            .prefix("Y "),
                                    );
                                });
                            }
                        }
                        for (label, point) in
                            [("Start", &mut parameters.from), ("End", &mut parameters.to)]
                        {
                            ui.horizontal(|ui| {
                                ui.label(label);
                                ui.add(
                                    egui::DragValue::new(&mut point.0)
                                        .range(-32768.0..=32768.0)
                                        .prefix("X "),
                                );
                                ui.add(
                                    egui::DragValue::new(&mut point.1)
                                        .range(-32768.0..=32768.0)
                                        .prefix("Y "),
                                );
                            });
                        }
                        ui.add(
                            egui::Slider::new(&mut parameters.style.width, 1.0..=512.0)
                                .text("Outline width"),
                        );
                        ui.checkbox(&mut parameters.style.antialias, "Antialias");
                        ui.checkbox(&mut parameters.style.filled, "Filled");
                        ui.horizontal(|ui| {
                            ui.label("Color");
                            ui.color_edit_button_srgba_unmultiplied(&mut parameters.style.color);
                        });
                        let mut outlined = parameters.style.outline.is_some();
                        if ui
                            .checkbox(&mut outlined, "Separate outline color")
                            .changed()
                        {
                            parameters.style.outline =
                                outlined.then_some(self.color.to_srgba_unmultiplied());
                        }
                        if let Some(outline) = &mut parameters.style.outline {
                            ui.color_edit_button_srgba_unmultiplied(outline);
                        }
                    });
                ui.horizontal(|ui| {
                    apply = ui.button("Apply (Enter)").clicked();
                    cancel = ui.button("Cancel (Escape)").clicked();
                });
            });
        if !open || cancel {
            self.finish_shape_edit(false);
            return;
        }
        if parameters != before || self.curve_controls != before_controls {
            self.shape_edit = Some(parameters);
            self.request_shape_edit_preview(parameters);
        }
        if apply {
            self.finish_shape_edit(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paint_rust::shapes::Parameters;
    fn draft(ctx: &egui::Context) -> Editor {
        let mut editor = Editor::blank(ctx);
        editor.document = Document::new(64, 64, [0; 4]).unwrap();
        editor.keep_shape_editable = true;
        editor.document.begin("Rectangle");
        editor.request_gesture_preview(Effect::Shape(Parameters {
            shape: Shape::Rectangle,
            from: (8.0, 8.0),
            to: (48.0, 48.0),
            style: ShapeStyle {
                paint_mode: None,
                antialias: true,
                outline: None,
                color: [20, 40, 80, 255],
                width: 2.0,
                filled: true,
            },
        }));
        assert!(editor.retain_shape());
        editor
    }
    #[test]
    fn retained_shape_commits_latest_preview_once_and_cancel_restores_source() {
        let ctx = egui::Context::default();
        let mut editor = draft(&ctx);
        editor.finish_stroke();
        assert!(editor.shape_edit.is_some());
        assert!(editor.document.editing());
        let mut parameters = editor.shape_edit.unwrap();
        parameters.to = (24.0, 24.0);
        editor.shape_edit = Some(parameters);
        editor.request_gesture_preview(Effect::Shape(parameters));
        editor.finish_shape_edit(true);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while editor.file_job.is_some() {
            editor.poll_file_job(&ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(editor.document.pixels()[16 * 64 + 16], [20, 40, 80, 255]);
        assert_eq!(editor.document.pixels()[40 * 64 + 40], [0; 4]);
        assert!(editor.document.can_undo());
        editor.document.undo();
        assert!(!editor.document.can_undo());
        assert!(
            editor
                .document
                .pixels()
                .iter()
                .all(|pixel| *pixel == [0; 4])
        );
        let mut editor = draft(&ctx);
        editor.finish_shape_edit(false);
        assert!(editor.shape_edit.is_none());
        assert!(editor.gesture_preview.is_none());
        assert!(!editor.document.editing());
        assert!(!editor.document.can_undo());
        assert!(
            editor
                .document
                .pixels()
                .iter()
                .all(|pixel| *pixel == [0; 4])
        );
    }
    #[test]
    fn center_handle_moves_geometry_in_image_coordinates() {
        let ctx = egui::Context::default();
        let mut editor = draft(&ctx);
        editor.zoom = 2.0;
        let center = Pos2::new(76.0, 76.0);
        let end = center + Vec2::new(20.0, 12.0);
        let mut frame = |events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 400.0))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        editor.shape_handles(
                            ui,
                            Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::splat(300.0)),
                        )
                    });
                },
            );
        };
        frame(vec![egui::Event::PointerMoved(center)]);
        frame(vec![egui::Event::PointerButton {
            pos: center,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }]);
        frame(vec![egui::Event::PointerMoved(end)]);
        frame(vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }]);
        let result = editor.shape_edit.unwrap();
        assert_eq!(result.from, (18.0, 14.0));
        assert_eq!(result.to, (58.0, 54.0));
        assert!(!editor.document.can_undo());
        editor.finish_shape_edit(false);
    }
    #[test]
    fn editable_curve_applies_latest_controls_and_clears_draft_state() {
        let ctx = egui::Context::default();
        let mut editor = draft(&ctx);
        let mut parameters = editor.shape_edit.unwrap();
        parameters.shape = Shape::Line;
        parameters.from = (8.0, 32.0);
        parameters.to = (56.0, 32.0);
        editor.shape_edit = Some(parameters);
        editor.curve_controls = Some([(8.0, 0.0), (56.0, 64.0)]);
        editor.request_shape_edit_preview(parameters);
        let expected = paint_rust::bezier::render(
            &vec![[0; 4]; 64 * 64],
            64,
            64,
            None,
            paint_rust::bezier::Parameters {
                points: [parameters.from, (8.0, 0.0), (56.0, 64.0), parameters.to],
                style: parameters.style,
            },
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
        assert!(matches!(
            editor.gesture_preview.as_ref().unwrap().effect,
            Effect::Bezier(_)
        ));
        editor.finish_shape_edit(true);
        assert!(editor.curve_controls.is_none());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while editor.file_job.is_some() {
            editor.poll_file_job(&ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(editor.document.pixels(), expected);
        assert_eq!(editor.document.history().count(), 1);
        editor.undo();
        assert!(!editor.document.dirty());
        assert!(
            editor
                .document
                .pixels()
                .iter()
                .all(|pixel| *pixel == [0; 4])
        );
    }
}
