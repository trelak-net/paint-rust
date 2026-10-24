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

struct Request {
    generation: u64,
    effect: Effect,
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
        let (requests, receive) = mpsc::channel::<Request>();
        let (send, results) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(mut request) = receive.recv() {
                while let Ok(newest) = receive.try_recv() {
                    request = newest;
                }
                if let Some(pixels) = effects::render(
                    &source,
                    width,
                    height,
                    selection.clone(),
                    request.effect,
                    &request.cancel,
                ) {
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
        preview.request(effect);
        preview
    }
    pub fn request(&mut self, effect: Effect) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.generation += 1;
        self.ready = false;
        let _ = self.requests.send(Request {
            generation: self.generation,
            effect,
            cancel: self.cancel.clone(),
        });
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
