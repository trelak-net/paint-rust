use super::*;

/// Reserve the full slider, numeric field and label before wrapping the toolbar.
pub(super) fn slider(ui: &mut egui::Ui, slider: egui::Slider<'_>, label: &str) -> egui::Response {
    let label_width = ui
        .painter()
        .layout_no_wrap(
            label.into(),
            egui::TextStyle::Body.resolve(ui.style()),
            ui.visuals().text_color(),
        )
        .size()
        .x;
    let width = ui.spacing().slider_width + 80.0 + label_width + 3.0 * ui.spacing().item_spacing.x;
    ui.allocate_ui_with_layout(
        egui::vec2(width, ui.spacing().interact_size.y),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| ui.add(slider.text(label)),
    )
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrapped_toolbar_keeps_complete_slider_labels_inside_the_window() {
        let ctx = egui::Context::default();
        let mut values = [0.5; 3];
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 220.0))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Tool options");
                        for (value, label) in
                            values
                                .iter_mut()
                                .zip(["Brush size", "Hardness", "Stroke opacity"])
                        {
                            slider(ui, egui::Slider::new(value, 0.0..=1.0), label);
                        }
                    });
                });
            },
        );
        let mut labels = 0;
        for clipped in output.shapes {
            if let egui::Shape::Text(text) = clipped.shape
                && ["Brush size", "Hardness", "Stroke opacity"]
                    .contains(&text.galley.job.text.as_str())
            {
                let bounds = Rect::from_min_size(text.pos, text.galley.size());
                assert!(
                    clipped.clip_rect.contains_rect(bounds),
                    "{} clipped: {bounds:?}",
                    text.galley.job.text
                );
                labels += 1;
            }
        }
        assert_eq!(labels, 3);
    }
}
