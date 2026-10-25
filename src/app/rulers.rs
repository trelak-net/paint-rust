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

fn physical_ticks(
    offset: f32,
    span: f32,
    zoom: f32,
    extent: u32,
    ppu: f64,
) -> Vec<(f32, f64, bool)> {
    if !offset.is_finite()
        || !span.is_finite()
        || !zoom.is_finite()
        || zoom <= 0.0
        || span <= 0.0
        || !ppu.is_finite()
        || ppu <= 0.0
    {
        return Vec::new();
    }
    let scale = f64::from(zoom) * ppu;
    let desired = 60.0 / scale;
    let magnitude = 10.0_f64.powf(desired.log10().floor());
    let major = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .find(|v| v * magnitude >= desired)
        .unwrap()
        * magnitude;
    let subdivisions: u32 = if major * scale / 5.0 >= 7.0 { 5 } else { 1 };
    let minor = major / f64::from(subdivisions);
    let first = (f64::from(offset.max(0.0)) / (minor * scale)).ceil() as u64;
    let last = ((f64::from(offset.max(0.0)) + f64::from(span))
        .min(f64::from(extent) * f64::from(zoom))
        / (minor * scale))
        .floor() as u64;
    (first..=last)
        .take(4096)
        .map(|i| {
            let value = i as f64 * minor;
            (
                (value * scale - f64::from(offset)) as f32,
                value,
                i % u64::from(subdivisions) == 0,
            )
        })
        .collect()
}
fn ruler_ticks(
    offset: f32,
    span: f32,
    zoom: f32,
    extent: u32,
    unit: paint_rust::resolution::Unit,
    dpi: f64,
) -> Vec<(f32, String, bool)> {
    if unit == paint_rust::resolution::Unit::Pixels {
        return ticks(offset, span, zoom, extent)
            .into_iter()
            .map(|(p, v, m)| (p, v.to_string(), m))
            .collect();
    }
    let ppu = unit.pixels_per_unit(dpi);
    let decimals = ((ppu * f64::from(zoom) / 60.0)
        .log10()
        .ceil()
        .clamp(0.0, 6.0)) as usize;
    physical_ticks(offset, span, zoom, extent, ppu)
        .into_iter()
        .map(|(p, v, m)| {
            let mut label = format!("{v:.decimals$}");
            if label.contains('.') {
                label = label.trim_end_matches('0').trim_end_matches('.').to_owned();
            }
            (p, label, m)
        })
        .collect()
}

const ZOOM_STEPS: &[f32] = &[
    0.01, 0.02, 0.03, 0.04, 0.05, 0.0625, 0.0833, 0.125, 0.1667, 0.25, 0.3333, 0.5, 0.6667, 1.0,
    1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0, 64.0,
];
impl Editor {
    pub(super) fn set_view_zoom(&mut self, zoom: f32) {
        let zoom = zoom.clamp(0.01, 64.0);
        let anchor = self.canvas_viewport.size().max(Vec2::ZERO) / 2.0;
        self.canvas_offset =
            ((self.canvas_offset + anchor) * (zoom / self.zoom) - anchor).max(Vec2::ZERO);
        self.zoom = zoom;
        self.fit_canvas = false;
    }
    pub(super) fn step_view_zoom(&mut self, increase: bool) {
        let zoom = if increase {
            ZOOM_STEPS
                .iter()
                .copied()
                .find(|z| *z > self.zoom + 0.00001)
                .unwrap_or(64.0)
        } else {
            ZOOM_STEPS
                .iter()
                .rev()
                .copied()
                .find(|z| *z < self.zoom - 0.00001)
                .unwrap_or(0.01)
        };
        self.set_view_zoom(zoom);
    }

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
            self.preferences.units.suffix(),
            egui::FontId::proportional(10.0),
            foreground,
        );
        let dpi = self.document.resolution();
        let h = painter.with_clip_rect(horizontal);
        for (position, value, major) in ruler_ticks(
            self.canvas_offset.x,
            self.canvas_viewport.width(),
            self.zoom,
            self.document.width,
            self.preferences.units,
            dpi.x,
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
                    value,
                    egui::FontId::proportional(9.0),
                    foreground,
                );
            }
        }
        let v = painter.with_clip_rect(vertical);
        for (position, value, major) in ruler_ticks(
            self.canvas_offset.y,
            self.canvas_viewport.height(),
            self.zoom,
            self.document.height,
            self.preferences.units,
            dpi.y,
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
                    value,
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
    fn physical_ruler_ticks_follow_resolution_and_keep_fractional_labels() {
        let values = physical_ticks(0.0, 1000.0, 2.0, 600, 300.0);
        assert!(values.iter().any(|(screen, value, major)| *major
            && (*value - 1.0).abs() < 1e-8
            && (*screen - 600.0).abs() < 0.01));
        let centimeters = physical_ticks(0.0, 1000.0, 1.0, 1000, 300.0 / 2.54);
        for (screen, value, _) in centimeters {
            assert!((f64::from(screen) - value * 300.0 / 2.54).abs() < 0.001);
        }
        let labels = ruler_ticks(
            0.0,
            1000.0,
            64.0,
            100,
            paint_rust::resolution::Unit::Inches,
            10000.0,
        );
        let major: Vec<_> = labels.iter().filter(|v| v.2).map(|v| v.1.clone()).collect();
        assert!(major.len() > 2);
        assert!(major.windows(2).all(|pair| pair[0] != pair[1]));
        assert!(physical_ticks(0.0, 100.0, 1.0, 100, f64::NAN).is_empty());
    }
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
