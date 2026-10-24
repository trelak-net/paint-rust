use eframe::egui;
use paint_rust::document::Document;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
};

struct Request {
    generation: u64,
    document: Document,
    cancel: Arc<AtomicBool>,
}

pub struct CanvasRenderer {
    requests: Sender<Request>,
    results: Receiver<(u64, egui::ColorImage)>,
    generation: u64,
    cancel: Arc<AtomicBool>,
    pub pending: bool,
}

impl CanvasRenderer {
    pub fn new(ctx: egui::Context) -> Self {
        let (requests, receive) = mpsc::channel::<Request>();
        let (send, results) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(mut request) = receive.recv() {
                while let Ok(newest) = receive.try_recv() {
                    request = newest;
                }
                let Some(pixels) = request.document.composite_cancellable(&request.cancel) else {
                    continue;
                };
                let mut colors = Vec::with_capacity(pixels.len());
                for chunk in pixels.chunks(4096) {
                    if request.cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    colors.extend(
                        chunk
                            .iter()
                            .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3])),
                    );
                }
                if request.cancel.load(Ordering::Relaxed) {
                    continue;
                }
                let size = [
                    request.document.width as usize,
                    request.document.height as usize,
                ];
                if send
                    .send((request.generation, egui::ColorImage::new(size, colors)))
                    .is_err()
                {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Self {
            requests,
            results,
            generation: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            pending: false,
        }
    }

    pub fn request(&mut self, document: &Document) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.generation += 1;
        self.pending = true;
        if self
            .requests
            .send(Request {
                generation: self.generation,
                document: document.snapshot(),
                cancel: self.cancel.clone(),
            })
            .is_err()
        {
            self.pending = false;
        }
    }

    pub fn poll(&mut self) -> Option<egui::ColorImage> {
        let mut latest = None;
        while let Ok((generation, image)) = self.results.try_recv() {
            if generation == self.generation {
                self.pending = false;
                latest = Some(image);
            }
        }
        latest
    }
}

impl Drop for CanvasRenderer {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_latest_generation_is_displayed_and_source_edits_are_isolated() {
        let mut renderer = CanvasRenderer::new(egui::Context::default());
        let mut document = Document::new(512, 512, [255; 4]).unwrap();
        renderer.request(&document);
        document.invert();
        renderer.request(&document);
        document.invert();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let image = loop {
            if let Some(image) = renderer.poll() {
                break image;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        assert_eq!(image.size, [512, 512]);
        assert!(image.pixels.iter().all(|p| *p == egui::Color32::BLACK));
        assert!(!renderer.pending);
        assert!(document.pixels().iter().all(|p| *p == [255; 4]));
    }
}
