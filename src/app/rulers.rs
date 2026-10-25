use super::*;

fn ticks(offset: f32, span: f32, zoom: f32, extent: u32) -> Vec<(f32, u32, bool)> {
    if !offset.is_finite() || !span.is_finite() || !zoom.is_finite() || zoom <= 0.0 || span <= 0.0 {
        return Vec::new();
    }
    let desired = (60.0 / f64::from(zoom)).max(1.0);
    let magnitude = 10.0_f64.powf(desired.log10().floor());
    let major = ([1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .find(|v| v * magnitude >= desired)
        .unwrap()
        * magnitude)
        .round()
        .max(1.0) as u32;
    let minor = if major >= 5 && major as f32 * zoom / 5.0 >= 7.0 {
        major / 5
    } else {
        major
    };
    let first = (offset.max(0.0) / zoom / minor as f32).ceil() as u32;
    let last = ((offset.max(0.0) + span) / zoom).min(extent as f32).floor() as u32 / minor;
    (first..=last)
        .take(4096)
        .filter_map(|i| {
            let value = i.checked_mul(minor)?;
            Some((
                value as f32 * zoom - offset,
                value,
                value.is_multiple_of(major),
            ))
        })
        .collect()
}

impl Editor {
    pub(super) fn zoom_to_selection(&mut self) {
        let Some(selection) = self.document.selection() else {
            self.fit_canvas = true;
            return;
        };
        if selection.width == 0
            || selection.height == 0
            || self.canvas_viewport.width() <= 0.0
            || self.canvas_viewport.height() <= 0.0
        {
            return;
        }
        self.zoom = (self.canvas_viewport.width() / selection.width as f32)
            .min(self.canvas_viewport.height() / selection.height as f32)
            .clamp(0.01, 64.0);
        self.canvas_offset = (Vec2::new(
            selection.x as f32 + selection.width as f32 / 2.0,
            selection.y as f32 + selection.height as f32 / 2.0,
        ) * self.zoom
            - self.canvas_viewport.size() / 2.0)
            .max(Vec2::ZERO);
        self.fit_canvas = false;
    }
    pub(super) fn canvas(&mut self, ui: &mut egui::Ui) {
        if !self.preferences.rulers {
            self.canvas_content(ui);
            return;
        }
        let area = ui.available_rect_before_wrap();
        let width = 42.0_f32.min(area.width());
        let height = 22.0_f32.min(area.height());
        let content = Rect::from_min_max(area.min + Vec2::new(width, height), area.max);
        ui.allocate_rect(area, Sense::hover());
        ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| {
            self.canvas_content(ui)
        });
        let horizontal = Rect::from_min_max(
            area.min + Vec2::new(width, 0.0),
            Pos2::new(area.right(), area.top() + height),
        );
        let vertical = Rect::from_min_max(
            area.min + Vec2::new(0.0, height),
            Pos2::new(area.left() + width, area.bottom()),
        );
        let painter = ui.painter().with_clip_rect(area);
        let background = ui.visuals().panel_fill;
        let foreground = ui.visuals().text_color();
        painter.rect_filled(horizontal, 0.0, background);
        painter.rect_filled(vertical, 0.0, background);
        painter.rect_filled(
            Rect::from_min_size(area.min, Vec2::new(width, height)),
            0.0,
            background,
        );
        painter.text(
            area.min + Vec2::new(5.0, 4.0),
            egui::Align2::LEFT_TOP,
            "px",
            egui::FontId::proportional(10.0),
            foreground,
        );
        let h = painter.with_clip_rect(horizontal);
        for (position, value, major) in ticks(
            self.canvas_offset.x,
            self.canvas_viewport.width(),
            self.zoom,
            self.document.width,
        ) {
            let x = self.canvas_viewport.left() + position;
            h.line_segment(
                [
                    Pos2::new(x, horizontal.bottom()),
                    Pos2::new(x, horizontal.bottom() - if major { 7.0 } else { 3.0 }),
                ],
                egui::Stroke::new(1.0_f32, foreground),
            );
            if major {
                h.text(
                    Pos2::new(x + 2.0, horizontal.top()),
                    egui::Align2::LEFT_TOP,
                    value.to_string(),
                    egui::FontId::proportional(9.0),
                    foreground,
                );
            }
        }
        let v = painter.with_clip_rect(vertical);
        for (position, value, major) in ticks(
            self.canvas_offset.y,
            self.canvas_viewport.height(),
            self.zoom,
            self.document.height,
        ) {
            let y = self.canvas_viewport.top() + position;
            v.line_segment(
                [
                    Pos2::new(vertical.right(), y),
                    Pos2::new(vertical.right() - if major { 7.0 } else { 3.0 }, y),
                ],
                egui::Stroke::new(1.0_f32, foreground),
            );
            if major {
                v.text(
                    Pos2::new(vertical.left() + 2.0, y + 2.0),
                    egui::Align2::LEFT_TOP,
                    value.to_string(),
                    egui::FontId::proportional(9.0),
                    foreground,
                );
            }
        }
        if let Some(pointer) = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| self.canvas_viewport.contains(*p))
        {
            let stroke = egui::Stroke::new(1.0_f32, ui.visuals().selection.stroke.color);
            h.line_segment(
                [
                    Pos2::new(pointer.x, horizontal.top()),
                    Pos2::new(pointer.x, horizontal.bottom()),
                ],
                stroke,
            );
            v.line_segment(
                [
                    Pos2::new(vertical.left(), pointer.y),
                    Pos2::new(vertical.right(), pointer.y),
                ],
                stroke,
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ticks_follow_zoom_and_scroll_without_dense_or_negative_labels() {
        for zoom in [0.01, 0.1, 1.0, 8.0, 64.0] {
            let values = ticks(13.0, 1000.0, zoom, 16384);
            assert!(values.len() < 200);
            for &(screen, value, _) in &values {
                assert!((0.0..=1000.1).contains(&screen));
                assert!((screen - (value as f32 * zoom - 13.0)).abs() < 0.01);
            }
            let major: Vec<_> = values.iter().filter(|p| p.2).collect();
            assert!(major.windows(2).all(|p| p[1].0 - p[0].0 >= 50.0));
        }
        assert!(ticks(0.0, 100.0, 0.0, 100).is_empty());
        assert!(ticks(0.0, 100.0, f32::NAN, 100).is_empty());
    }
}
