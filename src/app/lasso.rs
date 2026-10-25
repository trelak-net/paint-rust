use super::*;

#[derive(Default)]
pub(super) struct Lasso {
    points: Vec<(f64, f64)>,
    drawing: bool,
}
impl Lasso {
    pub(super) fn cancel(&mut self) {
        self.points.clear();
        self.drawing = false;
    }
}

impl Editor {
    pub(super) fn lasso_canvas(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: Rect,
        painter: &egui::Painter,
        temporary_pan: bool,
    ) {
        if self.tool != Tool::Lasso
            || !ui.is_enabled()
            || !ui.input(|i| i.focused)
            || self.panning
            || temporary_pan
        {
            self.lasso.cancel();
            return;
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
        if !self.suppress_stroke
            && response.is_pointer_button_down_on()
            && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
        {
            self.lasso.cancel();
            self.lasso.drawing = true;
        }
        if !self.lasso.drawing {
            return;
        }
        if let Some(pointer) = ui.input(|i| i.pointer.latest_pos()) {
            let local = (pointer - rect.min) / self.zoom;
            let point = (f64::from(local.x), f64::from(local.y));
            if self
                .lasso
                .points
                .last()
                .is_none_or(|p| (p.0 - point.0).hypot(p.1 - point.1) >= 0.5)
            {
                if self.lasso.points.len() == 4096 {
                    self.lasso.cancel();
                    self.status =
                        "Lasso exceeded 4096 points; the previous selection was retained".into();
                    return;
                }
                self.lasso.points.push(point);
            }
        }
        let points: Vec<Pos2> = self
            .lasso
            .points
            .iter()
            .map(|(x, y)| rect.min + egui::vec2(*x as f32, *y as f32) * self.zoom)
            .collect();
        if points.len() > 1 {
            for segment in points
                .windows(2)
                .map(|p| [p[0], p[1]])
                .chain(std::iter::once([*points.last().unwrap(), points[0]]))
            {
                painter.line_segment(segment, egui::Stroke::new(3.0_f32, Color32::BLACK));
                painter.line_segment(segment, egui::Stroke::new(1.0_f32, Color32::WHITE));
            }
        }
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            self.lasso.drawing = false;
            let points = std::mem::take(&mut self.lasso.points);
            if points.len() < 3 {
                return;
            }
            let (width, height) = (self.document.width, self.document.height);
            let previous = self.document.selection();
            let mode = self.selection_mode;
            self.start_cancellable_file_job("select lasso", move |cancel| {
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
    }
}
