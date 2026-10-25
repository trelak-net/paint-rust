use super::*;

pub(super) fn controls(
    ui: &mut egui::Ui,
    curves: &mut paint_rust::curves::Curves,
    channel: &mut usize,
) -> bool {
    let before = *curves;
    ui.horizontal(|ui| {
        ui.selectable_value(&mut curves.luminance, false, "RGB channels");
        ui.selectable_value(&mut curves.luminance, true, "Luminance");
    });
    if curves.luminance {
        ui.small("Changes brightness while retaining channel differences until clipping.");
    } else {
        let names = ["RGB", "Red", "Green", "Blue"];
        egui::ComboBox::from_label("Channel")
            .selected_text(names[*channel])
            .show_ui(ui, |ui| {
                for (index, name) in names.iter().enumerate() {
                    ui.selectable_value(channel, index, *name);
                }
            });
    }
    let luminance = curves.luminance;
    let curve = if luminance {
        &mut curves.luminance_curve
    } else {
        &mut curves.channels[*channel]
    };
    ui.menu_button("Presets", |ui| {
        for (label, points) in [
            ("Identity", &[][..]),
            ("Increase contrast", &[(64, 40), (192, 215)][..]),
            ("Lift shadows", &[(64, 96), (192, 208)][..]),
            ("Invert", &[(0, 255), (255, 0)][..]),
        ] {
            if ui.button(label).clicked() {
                *curve = Default::default();
                for &(x, y) in points {
                    if x == 0 {
                        curve.move_point(0, 0, y);
                    } else if x == 255 {
                        curve.move_point(curve.points().len() - 1, 255, y);
                    } else {
                        curve.insert(x, y);
                    }
                }
                ui.close();
            }
        }
    });
    let (response, painter) = ui.allocate_painter(Vec2::splat(256.0), Sense::click_and_drag());
    let rect = response.rect.shrink(5.0);
    let position = |x: u8, y: u8| {
        Pos2::new(
            rect.left() + f32::from(x) / 255.0 * rect.width(),
            rect.bottom() - f32::from(y) / 255.0 * rect.height(),
        )
    };
    let point_id = response.id.with((*channel, luminance));
    let mut selected = ui
        .data_mut(|data| data.get_temp::<usize>(point_id))
        .unwrap_or(0)
        .min(curve.points().len() - 1);
    if let Some(pointer) = response.interact_pointer_pos() {
        let x = ((pointer.x - rect.left()) / rect.width() * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
        let y = ((rect.bottom() - pointer.y) / rect.height() * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
        let nearest = curve
            .points()
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                position(a.0, a.1)
                    .distance_sq(pointer)
                    .total_cmp(&position(b.0, b.1).distance_sq(pointer))
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        if response.secondary_clicked()
            && position(curve.points()[nearest].0, curve.points()[nearest].1).distance(pointer)
                < 12.0
        {
            curve.remove(nearest);
            selected = selected.min(curve.points().len() - 1);
        } else if response.double_clicked() {
            if let Some(index) = curve.insert(x, y) {
                selected = index;
            }
        } else if response.drag_started() || response.clicked() {
            selected = nearest;
        }
        if response.dragged() {
            curve.move_point(selected, x, y);
        }
    }
    painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
    for step in 0..=4 {
        let fraction = step as f32 / 4.0;
        let x = egui::lerp(rect.x_range(), fraction);
        let y = egui::lerp(rect.y_range(), fraction);
        let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            stroke,
        );
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            stroke,
        );
    }
    let color = match if luminance { 0 } else { *channel } {
        1 => Color32::from_rgb(240, 90, 90),
        2 => Color32::from_rgb(70, 190, 100),
        3 => Color32::from_rgb(90, 140, 255),
        _ => ui.visuals().text_color(),
    };
    let line = curve
        .table()
        .iter()
        .enumerate()
        .map(|(x, y)| position(x as u8, *y))
        .collect();
    painter.add(egui::Shape::line(line, egui::Stroke::new(2.0_f32, color)));
    for (index, (x, y)) in curve.points().iter().enumerate() {
        painter.circle_filled(
            position(*x, *y),
            if index == selected { 5.0 } else { 3.0 },
            color,
        );
    }
    ui.small("Double-click: add point · Drag: move · Right-click: remove");
    egui::ComboBox::from_label("Control point")
        .selected_text(format!("{}", selected + 1))
        .show_ui(ui, |ui| {
            for index in 0..curve.points().len() {
                ui.selectable_value(&mut selected, index, format!("{}", index + 1));
            }
        });
    let (mut x, mut y) = curve.points()[selected];
    let x_changed = ui
        .add_enabled(
            selected > 0 && selected + 1 < curve.points().len(),
            egui::Slider::new(&mut x, 0..=255).text("Input"),
        )
        .changed();
    let y_changed = ui
        .add(egui::Slider::new(&mut y, 0..=255).text("Output"))
        .changed();
    if x_changed || y_changed {
        curve.move_point(selected, x, y);
    }
    ui.horizontal(|ui| {
        if ui.button("Add point").clicked() {
            let gap = curve
                .points()
                .windows(2)
                .max_by_key(|p| p[1].0 - p[0].0)
                .unwrap();
            if gap[1].0 - gap[0].0 > 1 {
                let x = gap[0].0 + (gap[1].0 - gap[0].0) / 2;
                if let Some(index) = curve.insert(x, curve.table()[usize::from(x)]) {
                    selected = index;
                }
            }
        }
        if ui
            .add_enabled(
                selected > 0 && selected + 1 < curve.points().len(),
                egui::Button::new("Remove point"),
            )
            .clicked()
        {
            curve.remove(selected);
            selected = selected.min(curve.points().len() - 1);
        }
        if ui.button("Reset channel").clicked() {
            *curve = Default::default();
            selected = 0;
        }
    });
    if ui.button("Reset all").clicked() {
        *curves = Default::default();
        selected = 0;
    }
    ui.data_mut(|data| data.insert_temp(point_id, selected));
    *curves != before
}
