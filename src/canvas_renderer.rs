use eframe::egui;
use paint_rust::document::Document;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
};

struct Request {
    generation: u64,
    base_generation: u64,
    document: Document,
    cancel: Arc<AtomicBool>,
}

pub enum Update {
    Full(egui::ColorImage),
    Partial {
        position: [usize; 2],
        image: egui::ColorImage,
    },
    Unchanged,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Bounds {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn changed_bounds(
    before: &Document,
    after: &Document,
    cancel: &AtomicBool,
) -> Result<Option<Bounds>, ()> {
    let full = Bounds {
        x: 0,
        y: 0,
        width: after.width,
        height: after.height,
    };
    if before.width != after.width
        || before.height != after.height
        || before.layers().len() != after.layers().len()
    {
        return Ok(Some(full));
    }
    let (mut left, mut top, mut right, mut bottom) = (after.width, after.height, 0, 0);
    for (before, after_layer) in before.layers().iter().zip(after.layers()) {
        if before.visible != after_layer.visible
            || before.opacity != after_layer.opacity
            || before.blend_mode != after_layer.blend_mode
        {
            return Ok(Some(full));
        }
        if !before.visible
            || before.opacity == 0
            || std::ptr::eq(before.pixels(), after_layer.pixels())
        {
            continue;
        }
        for (y, (before, after_row)) in before
            .pixels()
            .chunks(after.width as usize)
            .zip(after_layer.pixels().chunks(after.width as usize))
            .enumerate()
        {
            if cancel.load(Ordering::Relaxed) {
                return Err(());
            }
            if before == after_row {
                continue;
            }
            top = top.min(y as u32);
            bottom = bottom.max(y as u32 + 1);
            let start = before
                .iter()
                .zip(after_row)
                .position(|(a, b)| a != b)
                .expect("changed row");
            let end = before
                .iter()
                .zip(after_row)
                .rposition(|(a, b)| a != b)
                .expect("changed row");
            left = left.min(start as u32);
            right = right.max(end as u32 + 1);
        }
    }
    Ok((right > left && bottom > top).then_some(Bounds {
        x: left,
        y: top,
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    }))
}

fn render(request: &Request, previous: Option<&(u64, Document)>) -> Option<Update> {
    if request.cancel.load(Ordering::Relaxed) {
        return None;
    }
    let full = Bounds {
        x: 0,
        y: 0,
        width: request.document.width,
        height: request.document.height,
    };
    // Only patch an image the UI has actually consumed. A completed but superseded
    // worker result must never become the base of an incremental update.
    let bounds = if let Some((_, document)) =
        previous.filter(|(generation, _)| *generation == request.base_generation)
    {
        changed_bounds(document, &request.document, &request.cancel).ok()?
    } else {
        Some(full)
    };
    let Some(bounds) = bounds else {
        return Some(Update::Unchanged);
    };
    let pixels = request.document.composite_region(
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        &request.cancel,
    )?;
    let mut colors = Vec::with_capacity(pixels.len());
    for chunk in pixels.chunks(4096) {
        if request.cancel.load(Ordering::Relaxed) {
            return None;
        }
        colors.extend(
            chunk
                .iter()
                .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3])),
        );
    }
    let image = egui::ColorImage::new([bounds.width as usize, bounds.height as usize], colors);
    Some(if bounds == full {
        Update::Full(image)
    } else {
        Update::Partial {
            position: [bounds.x as usize, bounds.y as usize],
            image,
        }
    })
}

pub struct CanvasRenderer {
    requests: Sender<Request>,
    results: Receiver<(u64, Update)>,
    generation: u64,
    displayed_generation: u64,
    cancel: Arc<AtomicBool>,
    pub pending: bool,
}

impl CanvasRenderer {
    pub fn new(ctx: egui::Context) -> Self {
        let (requests, receive) = mpsc::channel::<Request>();
        let (send, results) = mpsc::channel();
        std::thread::spawn(move || {
            let mut previous = None;
            while let Ok(mut request) = receive.recv() {
                while let Ok(newest) = receive.try_recv() {
                    request = newest;
                }
                let Some(update) = render(&request, previous.as_ref()) else {
                    continue;
                };
                if request.cancel.load(Ordering::Relaxed) {
                    continue;
                }
                if send.send((request.generation, update)).is_err() {
                    break;
                }
                previous = Some((request.generation, request.document));
                ctx.request_repaint();
            }
        });
        Self {
            requests,
            results,
            generation: 0,
            displayed_generation: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            pending: false,
        }
    }

    pub fn invalidate(&mut self) {
        self.displayed_generation = 0;
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
                base_generation: self.displayed_generation,
                document: document.snapshot(),
                cancel: self.cancel.clone(),
            })
            .is_err()
        {
            self.pending = false;
        }
    }

    pub fn poll(&mut self) -> Option<Update> {
        let mut latest = None;
        while let Ok((generation, image)) = self.results.try_recv() {
            if generation == self.generation {
                self.pending = false;
                self.displayed_generation = generation;
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
    fn wait(renderer: &mut CanvasRenderer) -> Update {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(update) = renderer.poll() {
                return update;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    #[test]
    fn only_latest_generation_is_displayed_and_source_edits_are_isolated() {
        let mut renderer = CanvasRenderer::new(egui::Context::default());
        let mut document = Document::new(512, 512, [255; 4]).unwrap();
        renderer.request(&document);
        document.invert();
        renderer.request(&document);
        document.invert();
        let Update::Full(image) = wait(&mut renderer) else {
            panic!("full image required");
        };
        assert_eq!(image.size, [512, 512]);
        assert!(image.pixels.iter().all(|p| *p == egui::Color32::BLACK));
        assert!(!renderer.pending);
        assert!(document.pixels().iter().all(|p| *p == [255; 4]));
    }

    #[test]
    fn small_stroke_sends_a_patch_and_metadata_changes_send_a_full_image() {
        let mut renderer = CanvasRenderer::new(egui::Context::default());
        let mut document = Document::new(64, 64, [255; 4]).unwrap();
        renderer.request(&document);
        let Update::Full(mut displayed) = wait(&mut renderer) else {
            panic!("full image required");
        };
        document.begin("Stroke");
        document.stroke((10.0, 10.0), (15.0, 15.0), 3.0, [20, 40, 60, 128], false);
        document.commit();
        renderer.request(&document);
        let Update::Partial { position, image } = wait(&mut renderer) else {
            panic!("small patch required");
        };
        assert!(image.pixels.len() < 64 * 64 / 4);
        for y in 0..image.size[1] {
            for x in 0..image.size[0] {
                displayed[(position[0] + x, position[1] + y)] = image[(x, y)];
            }
        }
        assert_eq!(
            displayed.pixels,
            document
                .composite()
                .iter()
                .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
                .collect::<Vec<_>>()
        );
        document.set_visibility(0, false);
        renderer.request(&document);
        assert!(matches!(wait(&mut renderer), Update::Full(_)));
    }

    #[test]
    fn patch_base_must_match_the_image_consumed_by_the_ui() {
        let before = Document::new(64, 64, [255; 4]).unwrap();
        let mut after = before.snapshot();
        after.begin("Stroke");
        after.stroke((10.0, 10.0), (15.0, 15.0), 3.0, [0; 4], false);
        after.commit();
        let request = Request {
            generation: 3,
            base_generation: 1,
            document: after,
            cancel: Arc::new(AtomicBool::new(false)),
        };
        assert!(matches!(
            render(&request, Some(&(2, before))),
            Some(Update::Full(_))
        ));
    }
}
