use eframe::egui;
use paint_rust::{
    document::{Pixel, Selection},
    effects::{self, Effect},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
};

pub struct TextInput {
    pub layout: paint_rust::text::TextLayout,
    pub font: ab_glyph::FontArc,
    pub content: String,
    pub origin: (f32, f32),
    pub size: f32,
    pub color: Pixel,
}

enum Input {
    Effect(Effect),
    Text(TextInput),
}

struct Request {
    generation: u64,
    input: Input,
    cancel: Arc<AtomicBool>,
}

pub struct EffectPreview {
    requests: Sender<Request>,
    results: Receiver<(u64, Vec<Pixel>)>,
    generation: u64,
    cancel: Arc<AtomicBool>,
    pub ready: bool,
}
impl EffectPreview {
    pub fn new(
        ctx: egui::Context,
        source: Arc<Vec<Pixel>>,
        width: u32,
        height: u32,
        selection: Option<Selection>,
        effect: Effect,
    ) -> Self {
        Self::start(ctx, source, width, height, selection, Input::Effect(effect))
    }

    pub fn new_text(
        ctx: egui::Context,
        source: Arc<Vec<Pixel>>,
        width: u32,
        height: u32,
        selection: Option<Selection>,
        text: TextInput,
    ) -> Self {
        Self::start(ctx, source, width, height, selection, Input::Text(text))
    }

    fn start(
        ctx: egui::Context,
        source: Arc<Vec<Pixel>>,
        width: u32,
        height: u32,
        selection: Option<Selection>,
        input: Input,
    ) -> Self {
        let (requests, receive) = mpsc::channel::<Request>();
        let (send, results) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(mut request) = receive.recv() {
                while let Ok(newest) = receive.try_recv() {
                    request = newest;
                }
                let pixels = match request.input {
                    Input::Effect(effect) => effects::render(
                        &source,
                        width,
                        height,
                        selection.clone(),
                        effect,
                        &request.cancel,
                    ),
                    Input::Text(text) => {
                        if request.cancel.load(Ordering::Relaxed) {
                            continue;
                        }
                        let mut pixels = source.as_ref().clone();
                        paint_rust::text::render_cancellable(
                            &mut pixels,
                            width,
                            height,
                            selection.clone(),
                            paint_rust::text::TextStyle {
                                layout: text.layout,
                                font: &text.font,
                                text: &text.content,
                                origin: text.origin,
                                size: text.size,
                                color: text.color,
                            },
                            &request.cancel,
                        )
                        .then_some(pixels)
                    }
                };
                if let Some(pixels) = pixels {
                    if send.send((request.generation, pixels)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            }
        });
        let mut preview = Self {
            requests,
            results,
            generation: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            ready: false,
        };
        preview.request_input(input);
        preview
    }
    pub fn request(&mut self, effect: Effect) {
        self.request_input(Input::Effect(effect));
    }
    pub fn request_text(&mut self, text: TextInput) {
        self.request_input(Input::Text(text));
    }
    fn request_input(&mut self, input: Input) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.generation += 1;
        self.ready = false;
        let _ = self.requests.send(Request {
            generation: self.generation,
            input,
            cancel: self.cancel.clone(),
        });
    }
    /// Wait on a background thread for the final requested generation.
    pub fn wait(self, cancel: &AtomicBool) -> Result<Vec<Pixel>, String> {
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(paint_rust::files::CANCELLED.into());
            }
            match self
                .results
                .recv_timeout(std::time::Duration::from_millis(10))
            {
                Ok((generation, pixels)) if generation == self.generation => {
                    if cancel.load(Ordering::Relaxed) {
                        return Err(paint_rust::files::CANCELLED.into());
                    }
                    return Ok(pixels);
                }
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Preview worker stopped before completing the image".into());
                }
            }
        }
    }

    pub fn poll(&mut self) -> Option<Vec<Pixel>> {
        let mut latest = None;
        while let Ok((generation, pixels)) = self.results.try_recv() {
            if generation == self.generation {
                self.ready = true;
                latest = Some(pixels);
            }
        }
        latest
    }
}
impl Drop for EffectPreview {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_worker_publishes_only_the_latest_content() {
        let definitions = egui::FontDefinitions::default();
        let name = &definitions.families[&egui::FontFamily::Proportional][0];
        let font =
            ab_glyph::FontArc::try_from_vec(definitions.font_data[name].font.to_vec()).unwrap();
        let input = |content: &str| TextInput {
            layout: Default::default(),
            font: font.clone(),
            content: content.into(),
            origin: (1.0, 1.0),
            size: 32.0,
            color: [0, 0, 0, 255],
        };
        let mut preview = EffectPreview::new_text(
            egui::Context::default(),
            Arc::new(vec![[255; 4]; 128 * 128]),
            128,
            128,
            None,
            input("Obsolete text"),
        );
        preview.request_text(input(""));
        let pixels = preview.wait(&AtomicBool::new(false)).unwrap();
        assert!(pixels.iter().all(|pixel| *pixel == [255; 4]));
    }

    #[test]
    fn waiting_for_final_preview_can_be_cancelled() {
        let preview = EffectPreview::new(
            egui::Context::default(),
            Arc::new(vec![[100; 4]; 128 * 128]),
            128,
            128,
            None,
            Effect::BoxBlur(64),
        );
        assert_eq!(
            preview.wait(&AtomicBool::new(true)).unwrap_err(),
            paint_rust::files::CANCELLED
        );
    }

    #[test]
    fn only_the_latest_request_becomes_ready() {
        let mut preview = EffectPreview::new(
            egui::Context::default(),
            Arc::new(vec![[100, 100, 100, 255]; 128 * 128]),
            128,
            128,
            None,
            Effect::BoxBlur(64),
        );
        preview.request(Effect::BrightnessContrast {
            brightness: 80,
            contrast: 0,
        });
        preview.request(Effect::BrightnessContrast {
            brightness: 10,
            contrast: 0,
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(pixels) = preview.poll() {
                assert_eq!(pixels[0], [126, 126, 126, 255]);
                assert!(preview.ready);
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "preview worker did not finish"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}
