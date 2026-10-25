use super::*;
pub(super) fn colors(ui: &mut egui::Ui, colors: &mut [[u8; 4]; 2]) -> bool {
    let mut changed = false;
    for (label, pixel) in ["Low / interior color", "High / exterior color"]
        .into_iter()
        .zip(colors)
    {
        ui.label(label);
        let mut rgba = Rgba::from_rgba_unmultiplied(pixel[0], pixel[1], pixel[2], pixel[3]);
        if rgba.edit(ui).changed() {
            *pixel = rgba.to_srgba_unmultiplied();
            changed = true;
        }
    }
    changed
}
pub(super) fn fractal(
    ui: &mut egui::Ui,
    settings: &mut paint_rust::effects::FractalSettings,
    julia: bool,
) -> bool {
    let mut changed = false;
    changed |= ui
        .add(egui::Slider::new(&mut settings.iterations, 1..=512).text("Iterations"))
        .changed();
    changed |= ui
        .add(
            egui::Slider::new(&mut settings.zoom, 0.1..=1000.0)
                .logarithmic(true)
                .text("Zoom"),
        )
        .changed();
    for (label, value) in ["Center real", "Center imaginary"]
        .into_iter()
        .zip(&mut settings.center)
    {
        changed |= ui
            .add(
                egui::DragValue::new(value)
                    .range(-4.0..=4.0)
                    .speed(0.001)
                    .prefix(format!("{label} ")),
            )
            .changed();
    }
    changed |= ui
        .add(egui::Slider::new(&mut settings.angle, -180.0..=180.0).text("Angle (degrees)"))
        .changed();
    if julia {
        ui.collapsing("Julia constant", |ui| {
            for (label, value) in ["Real", "Imaginary"].into_iter().zip(&mut settings.julia) {
                changed |= ui
                    .add(
                        egui::DragValue::new(value)
                            .range(-2.0..=2.0)
                            .speed(0.001)
                            .prefix(format!("{label} ")),
                    )
                    .changed();
            }
        });
    }
    changed |= colors(ui, &mut settings.colors);
    ui.small("Replaces selected pixels. More iterations increase detail and rendering time.");
    changed
}
