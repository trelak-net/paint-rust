use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

pub(super) struct Histogram {
    receiver: mpsc::Receiver<[[u32; 256]; 4]>,
    cancel: Arc<AtomicBool>,
    bins: Option<[[u32; 256]; 4]>,
    logarithmic: bool,
}
impl Histogram {
    pub fn new(
        ctx: egui::Context,
        source: Arc<Vec<[u8; 4]>>,
        width: u32,
        selection: Option<Selection>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        std::thread::spawn(move || {
            if let Some(bins) = paint_rust::levels::histogram_with_luminance(
                &source,
                width,
                selection.as_ref(),
                &worker_cancel,
            ) {
                let _ = sender.send(bins);
                ctx.request_repaint();
            }
        });
        Self {
            receiver,
            cancel,
            bins: None,
            logarithmic: false,
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui, channel: usize, output: bool) {
        if let Ok(bins) = self.receiver.try_recv() {
            self.bins = Some(bins);
        }
        ui.separator();
        ui.label(format!(
            "{} histogram — {}",
            if output { "Output" } else { "Input" },
            ["RGB", "Red", "Green", "Blue", "Luminance"][channel]
        ));
        ui.small("Selected nontransparent pixels");
        let Some(bins) = &self.bins else {
            ui.label("Calculating histogram…");
            return;
        };
        let data: [u32; 256] = if channel == 0 {
            std::array::from_fn(|i| bins[..3].iter().map(|c| c[i]).sum())
        } else {
            bins[channel - 1]
        };
        ui.checkbox(&mut self.logarithmic, "Logarithmic height");
        let height = |count: u32| {
            if self.logarithmic {
                (f64::from(count) + 1.0).ln() as f32
            } else {
                count as f32
            }
        };
        let max = height(*data.iter().max().unwrap()).max(1.0);
        let (response, painter) = ui.allocate_painter(Vec2::new(256.0, 90.0), Sense::hover());
        let rect = response.rect;
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        let color = match channel {
            1 => Color32::from_rgb(240, 90, 90),
            2 => Color32::from_rgb(70, 190, 100),
            3 => Color32::from_rgb(90, 140, 255),
            _ => ui.visuals().text_color(),
        };
        for (value, count) in data.iter().enumerate() {
            let x = rect.left() + value as f32 / 256.0 * rect.width();
            let top = rect.bottom() - height(*count) / max * rect.height();
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(x, top),
                    Pos2::new(x + rect.width() / 256.0, rect.bottom()),
                ),
                0.0,
                color,
            );
        }
        if let Some(pointer) = response.hover_pos() {
            let value = ((pointer.x - rect.left()) / rect.width() * 256.0)
                .floor()
                .clamp(0.0, 255.0) as usize;
            response.on_hover_text(format!("Value {value}: {} samples", data[value]));
        }
        ui.horizontal(|ui| {
            ui.label("0");
            ui.add_space(95.0);
            ui.label("128");
            ui.add_space(85.0);
            ui.label("255");
        });
        let count: u32 = bins[0].iter().sum();
        ui.small(format!(
            "{count} pixels · RGB view combines three channel counts"
        ));
    }
}
impl Drop for Histogram {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
