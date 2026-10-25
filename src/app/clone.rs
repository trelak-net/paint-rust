use super::*;
impl Editor {
    pub(super) fn clone_source(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        rect: Rect,
        painter: &egui::Painter,
        temporary_pan: bool,
    ) {
        if self.tool != Tool::Clone {
            return;
        }
        if ui.is_enabled()
            && !temporary_pan
            && !self.panning
            && self.last_point.is_none()
            && ui.input(|i| i.modifiers.ctrl)
            && response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let point = (pos - rect.min) / self.zoom;
            self.clone_anchor = Some((
                self.active_document_id,
                (point.x.floor() as i32, point.y.floor() as i32),
            ));
            self.clone_offset = None;
        }
        if let Some((id, anchor)) = self.clone_anchor
            && id == self.active_document_id
        {
            let point =
                if let (Some(offset), Some(pointer)) = (self.clone_offset, response.hover_pos()) {
                    pointer + Vec2::new(offset.0 as f32, offset.1 as f32) * self.zoom
                } else {
                    rect.min + Vec2::new(anchor.0 as f32 + 0.5, anchor.1 as f32 + 0.5) * self.zoom
                };
            for stroke in [
                egui::Stroke::new(3.0_f32, Color32::BLACK),
                egui::Stroke::new(1.0_f32, Color32::WHITE),
            ] {
                painter.line_segment(
                    [point - Vec2::new(6.0, 0.0), point + Vec2::new(6.0, 0.0)],
                    stroke,
                );
                painter.line_segment(
                    [point - Vec2::new(0.0, 6.0), point + Vec2::new(0.0, 6.0)],
                    stroke,
                );
            }
        }
    }
}
