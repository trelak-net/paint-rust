use super::*;
use paint_rust::affine::FreeTransform;

#[derive(Clone, Copy)]
pub(super) enum Handle {
    Move,
    Scale(Vec2),
    Rotate,
}

#[derive(Clone, Copy)]
pub(super) struct TransformDrag {
    handle: Handle,
    start: Pos2,
    center: Pos2,
    half_size: Vec2,
    initial: FreeTransform,
    max_scale: f32,
    max_offset: f32,
}

fn rotate(vector: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.to_radians().sin_cos();
    egui::vec2(
        cos * vector.x - sin * vector.y,
        sin * vector.x + cos * vector.y,
    )
}

impl TransformDrag {
    fn update(self, pointer: Pos2, shift: bool) -> FreeTransform {
        let mut result = self.initial;
        match self.handle {
            Handle::Move => {
                let mut delta = pointer - self.start;
                if shift {
                    if delta.x.abs() > delta.y.abs() {
                        delta.y = 0.0;
                    } else {
                        delta.x = 0.0;
                    }
                }
                result.offset_x =
                    (result.offset_x + delta.x).clamp(-self.max_offset, self.max_offset);
                result.offset_y =
                    (result.offset_y + delta.y).clamp(-self.max_offset, self.max_offset);
            }
            Handle::Scale(sign) => {
                let local = rotate(pointer - self.start, -self.initial.angle);
                result.scale_x = (self.initial.scale_x + local.x * sign.x / self.half_size.x)
                    .clamp(0.01, self.max_scale);
                result.scale_y = (self.initial.scale_y + local.y * sign.y / self.half_size.y)
                    .clamp(0.01, self.max_scale);
                if shift {
                    let minimum = (0.01 / self.initial.scale_x).max(0.01 / self.initial.scale_y);
                    let maximum = (self.max_scale / self.initial.scale_x)
                        .min(self.max_scale / self.initial.scale_y);
                    let factor = (result.scale_x / self.initial.scale_x)
                        .max(result.scale_y / self.initial.scale_y)
                        .clamp(minimum, maximum);
                    result.scale_x = (self.initial.scale_x * factor).clamp(0.01, self.max_scale);
                    result.scale_y = (self.initial.scale_y * factor).clamp(0.01, self.max_scale);
                }
            }
            Handle::Rotate => {
                let before = self.start - self.center;
                let after = pointer - self.center;
                if before.length_sq() > 0.0001 && after.length_sq() > 0.0001 {
                    let delta = (after.y.atan2(after.x) - before.y.atan2(before.x)).to_degrees();
                    result.angle = (self.initial.angle + delta + 180.0).rem_euclid(360.0) - 180.0;
                    if shift {
                        result.angle = (result.angle / 15.0).round() * 15.0;
                    }
                }
            }
        }
        result
    }
}

impl Editor {
    pub(super) fn transform_handles(&mut self, ui: &egui::Ui, rect: Rect, painter: &egui::Painter) {
        let boundary = self
            .selection_transform
            .as_ref()
            .map(|dialog| dialog.handle_state());
        let parameters = boundary
            .as_ref()
            .map(|(_, parameters, _)| *parameters)
            .or_else(|| {
                self.effect_dialog
                    .as_ref()
                    .and_then(|dialog| match dialog.effect {
                        Effect::FreeTransform(p) | Effect::LayerTransform(p) => Some(p),
                        _ => None,
                    })
            });
        let Some(parameters) = parameters else {
            self.transform_drag = None;
            return;
        };
        let whole_layer = self
            .effect_dialog
            .as_ref()
            .is_some_and(|dialog| matches!(dialog.effect, Effect::LayerTransform(_)));
        let selection = if whole_layer {
            None
        } else {
            boundary
                .as_ref()
                .map(|(selection, _, _)| selection.clone())
                .or_else(|| self.document.selection())
        };
        let (origin, size) = selection
            .as_ref()
            .map(|s| {
                (
                    egui::pos2(s.x as f32, s.y as f32),
                    egui::vec2(s.width as f32, s.height as f32),
                )
            })
            .unwrap_or((
                Pos2::ZERO,
                egui::vec2(self.document.width as f32, self.document.height as f32),
            ));
        if size.x <= 0.0 || size.y <= 0.0 {
            return;
        }
        let half_size = size / 2.0;
        let center = origin + half_size + egui::vec2(parameters.offset_x, parameters.offset_y);
        let scaled = half_size * egui::vec2(parameters.scale_x, parameters.scale_y);
        let to_screen = |point: Pos2| rect.min + point.to_vec2() * self.zoom;
        let signs = [
            egui::vec2(-1.0, -1.0),
            egui::vec2(1.0, -1.0),
            egui::vec2(1.0, 1.0),
            egui::vec2(-1.0, 1.0),
        ];
        let corners = signs.map(|sign| to_screen(center + rotate(scaled * sign, parameters.angle)));
        // Keep the rotation control inside the box so canvas-edge selections can use it.
        let rotation_local = egui::vec2(0.0, -scaled.y + (24.0 / self.zoom).min(scaled.y * 0.5));
        let rotation = to_screen(center + rotate(rotation_local, parameters.angle));
        for index in 0..4 {
            let segment = [corners[index], corners[(index + 1) % 4]];
            painter.line_segment(segment, egui::Stroke::new(3.0_f32, Color32::BLACK));
            painter.line_segment(segment, egui::Stroke::new(1.0_f32, Color32::WHITE));
            let handle = Rect::from_center_size(corners[index], Vec2::splat(8.0));
            painter.rect_filled(handle, 1.0, Color32::WHITE);
            painter.rect_stroke(
                handle,
                1.0,
                egui::Stroke::new(1.0_f32, Color32::BLACK),
                egui::StrokeKind::Inside,
            );
        }
        painter.circle_filled(rotation, 5.0, Color32::from_rgb(42, 113, 224));
        painter.circle_stroke(rotation, 5.0, egui::Stroke::new(1.0_f32, Color32::WHITE));
        let Some(pointer) = ui.input(|i| i.pointer.latest_pos()) else {
            return;
        };
        let image_pointer = Pos2::ZERO + (pointer - rect.min) / self.zoom;
        let on_canvas = painter.clip_rect().contains(pointer)
            && ui.ctx().layer_id_at(pointer) == Some(ui.layer_id());
        let hit = if rotation.distance(pointer) <= 9.0 {
            Some(Handle::Rotate)
        } else if let Some(index) = corners
            .iter()
            .position(|corner| corner.distance(pointer) <= 9.0)
        {
            Some(Handle::Scale(signs[index]))
        } else {
            let local = rotate(image_pointer - center, -parameters.angle);
            (local.x.abs() <= scaled.x && local.y.abs() <= scaled.y).then_some(Handle::Move)
        };
        if on_canvas && let Some(handle) = hit {
            ui.ctx().set_cursor_icon(match handle {
                Handle::Move => egui::CursorIcon::Grab,
                Handle::Scale(_) => egui::CursorIcon::ResizeNwSe,
                Handle::Rotate => egui::CursorIcon::Crosshair,
            });
            if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
                self.transform_drag = Some(TransformDrag {
                    handle,
                    start: image_pointer,
                    center,
                    half_size,
                    initial: parameters,
                    max_scale: if boundary.is_some() { 64.0 } else { 4.0 },
                    max_offset: if boundary.is_some() { 1e6 } else { 16384.0 },
                });
            }
        }
        if let Some(drag) = self.transform_drag {
            let constrained = ui.input(|i| i.modifiers.shift)
                || (matches!(drag.handle, Handle::Scale(_))
                    && boundary.as_ref().is_some_and(|(_, _, locked)| *locked));
            let changed = drag.update(image_pointer, constrained);
            if changed != parameters {
                if let Some(dialog) = &mut self.selection_transform {
                    dialog.update_from_handles(changed);
                } else if let Some(dialog) = &mut self.effect_dialog {
                    dialog.effect = if whole_layer {
                        Effect::LayerTransform(changed)
                    } else {
                        Effect::FreeTransform(changed)
                    };
                    dialog.preview.request(dialog.effect);
                }
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if !ui.input(|i| i.pointer.primary_down()) {
                self.transform_drag = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn drag(handle: Handle, start: Pos2) -> TransformDrag {
        TransformDrag {
            handle,
            start,
            center: egui::pos2(50.0, 50.0),
            half_size: Vec2::splat(20.0),
            initial: FreeTransform::default(),
            max_scale: 4.0,
            max_offset: 16384.0,
        }
    }
    #[test]
    fn translation_and_shift_constraint_use_original_gesture_anchor() {
        let drag = drag(Handle::Move, egui::pos2(40.0, 40.0));
        let first = drag.update(egui::pos2(45.0, 43.0), false);
        assert_eq!((first.offset_x, first.offset_y), (5.0, 3.0));
        let next = drag.update(egui::pos2(50.0, 42.0), true);
        assert_eq!((next.offset_x, next.offset_y), (10.0, 0.0));
    }
    #[test]
    fn corner_scaling_and_rotation_keep_pivot_fixed() {
        let scale = drag(Handle::Scale(Vec2::splat(1.0)), egui::pos2(70.0, 70.0));
        let result = scale.update(egui::pos2(90.0, 80.0), false);
        assert_eq!((result.scale_x, result.scale_y), (2.0, 1.5));
        let uniform = scale.update(egui::pos2(90.0, 80.0), true);
        assert_eq!((uniform.scale_x, uniform.scale_y), (2.0, 2.0));
        let rotate = drag(Handle::Rotate, egui::pos2(50.0, 30.0));
        assert_eq!(rotate.update(egui::pos2(70.0, 50.0), false).angle, 90.0);
        assert_eq!((result.offset_x, result.offset_y), (0.0, 0.0));
        let offset_grab = drag(Handle::Scale(Vec2::splat(1.0)), egui::pos2(73.0, 72.0));
        assert_eq!(
            offset_grab.update(egui::pos2(73.0, 72.0), false).scale_x,
            1.0
        );
    }
}
