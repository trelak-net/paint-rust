use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum Mode {
    #[default]
    Free,
    Ratio,
    Size,
}
impl Mode {
    fn label(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Ratio => "Fixed ratio",
            Self::Size => "Fixed size",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct Options {
    pub mode: Mode,
    pub width: u32,
    pub height: u32,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            mode: Mode::Free,
            width: 1,
            height: 1,
        }
    }
}
impl Options {
    pub fn valid(&self) -> bool {
        (1..=16384).contains(&self.width) && (1..=16384).contains(&self.height)
    }
    pub fn bounds(
        &self,
        origin: (f32, f32),
        point: (f32, f32),
        size: (u32, u32),
    ) -> (u32, u32, u32, u32) {
        let (ox, oy) = (
            origin.0.clamp(0.0, size.0 as f32),
            origin.1.clamp(0.0, size.1 as f32),
        );
        let (dx, dy) = (point.0 - ox, point.1 - oy);
        let (ox, oy) = if self.mode == Mode::Size {
            (ox.floor(), oy.floor())
        } else {
            (ox, oy)
        };
        let (sx, sy) = (
            if dx < 0.0 { -1.0 } else { 1.0 },
            if dy < 0.0 { -1.0 } else { 1.0 },
        );
        let (w, h) = match self.mode {
            Mode::Free => (dx.abs(), dy.abs()),
            Mode::Size => (self.width as f32, self.height as f32),
            Mode::Ratio => {
                let ratio = self.width as f32 / self.height.max(1) as f32;
                let w = dx.abs().max(dy.abs() * ratio);
                let h = w / ratio;
                let available_x = if sx < 0.0 { ox } else { size.0 as f32 - ox };
                let available_y = if sy < 0.0 { oy } else { size.1 as f32 - oy };
                let scale = (available_x / w.max(f32::MIN_POSITIVE))
                    .min(available_y / h.max(f32::MIN_POSITIVE))
                    .min(1.0);
                (w * scale, h * scale)
            }
        };
        let (ex, ey) = (ox + sx * w, oy + sy * h);
        let x = ox.min(ex).floor().clamp(0.0, size.0 as f32) as u32;
        let y = oy.min(ey).floor().clamp(0.0, size.1 as f32) as u32;
        let right = ox.max(ex).ceil().clamp(0.0, size.0 as f32) as u32;
        let bottom = oy.max(ey).ceil().clamp(0.0, size.1 as f32) as u32;
        (x, y, right - x, bottom - y)
    }
}
impl Editor {
    pub(super) fn selection_draw_options(&mut self, ui: &mut egui::Ui) {
        let options = &mut self.general_settings.selection_draw;
        egui::ComboBox::from_label("Draw mode")
            .selected_text(options.mode.label())
            .show_ui(ui, |ui| {
                for mode in [Mode::Free, Mode::Ratio, Mode::Size] {
                    ui.selectable_value(&mut options.mode, mode, mode.label());
                }
            });
        if options.mode != Mode::Free {
            ui.label("Width");
            ui.add(egui::DragValue::new(&mut options.width).range(1..=16384));
            ui.label("Height");
            ui.add(egui::DragValue::new(&mut options.height).range(1..=16384));
            if ui.button("Swap dimensions").clicked() {
                std::mem::swap(&mut options.width, &mut options.height);
            }
            ui.small(if options.mode == Mode::Size {
                "Pixels; clipped at canvas edges"
            } else {
                "Ratio; pixel bounds rounded outward"
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ratio_bounds_cover_all_drag_directions_and_shrink_at_edges() {
        let options = Options {
            mode: Mode::Ratio,
            width: 4,
            height: 3,
        };
        for (point, expected) in [
            ((70.0, 55.0), (50, 50, 20, 15)),
            ((30.0, 55.0), (30, 50, 20, 15)),
            ((70.0, 45.0), (50, 35, 20, 15)),
            ((30.0, 45.0), (30, 35, 20, 15)),
        ] {
            assert_eq!(options.bounds((50.0, 50.0), point, (100, 100)), expected);
        }
        assert_eq!(
            options.bounds((80.0, 80.0), (150.0, 150.0), (100, 100)),
            (80, 80, 20, 15)
        );
        assert_eq!(
            options.bounds((50.0, 50.0), (50.0, 50.0), (100, 100)),
            (50, 50, 0, 0)
        );
    }
    #[test]
    fn fixed_size_click_and_reverse_drag_are_clipped() {
        let options = Options {
            mode: Mode::Size,
            width: 12,
            height: 8,
        };
        assert_eq!(
            options.bounds((20.7, 20.2), (20.7, 20.2), (100, 100)),
            (20, 20, 12, 8)
        );
        assert_eq!(
            options.bounds((20.0, 20.0), (20.0, 20.0), (100, 100)),
            (20, 20, 12, 8)
        );
        assert_eq!(
            options.bounds((20.0, 20.0), (19.0, 19.0), (100, 100)),
            (8, 12, 12, 8)
        );
        assert_eq!(
            options.bounds((95.0, 95.0), (99.0, 99.0), (100, 100)),
            (95, 95, 5, 5)
        );
    }
}
