use eframe::egui;
use paint_rust::{
    document::Document,
    files::{self, RasterOptions},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
};

struct Request {
    generation: u64,
    options: RasterOptions,
    cancel: Arc<AtomicBool>,
}
pub struct EncodedPreview {
    pub bytes: Arc<Vec<u8>>,
    pub image: egui::ColorImage,
}
pub struct RasterPreview {
    requests: Sender<Request>,
    results: Receiver<(u64, Result<EncodedPreview, String>)>,
    generation: u64,
    cancel: Arc<AtomicBool>,
    pub ready: bool,
}
impl RasterPreview {
    pub fn new(ctx: egui::Context, document: Document, options: RasterOptions) -> Self {
        let (requests, receive) = mpsc::channel::<Request>();
        let (send, results) = mpsc::channel();
        std::thread::spawn(move || {
            let mut composite = None;
            while let Ok(mut request) = receive.recv() {
                while let Ok(newest) = receive.try_recv() {
                    request = newest;
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if composite.is_none() {
                        composite = Some(
                            document
                                .composite_cancellable(&request.cancel)
                                .ok_or(files::CANCELLED)?,
                        );
                    }
                    let bytes = files::encode_raster_bytes(
                        composite.as_ref().unwrap(),
                        (document.width, document.height),
                        request.options,
                        &request.cancel,
                    )?;
                    if request.cancel.load(Ordering::Relaxed) {
                        return Err(files::CANCELLED.into());
                    }
                    let decoded = if let Some(format) = request
                        .options
                        .format
                        .image_format()
                        .filter(|_| request.options.format != files::RasterFormat::Avif)
                    {
                        let mut reader =
                            image::ImageReader::with_format(std::io::Cursor::new(&bytes), format);
                        let mut limits = image::Limits::default();
                        limits.max_alloc = Some(256 * 1024 * 1024);
                        limits.max_image_width = Some(document.width);
                        limits.max_image_height = Some(document.height);
                        reader.limits(limits);
                        reader.decode().map_err(|error| error.to_string())?
                    } else {
                        let decoded = files::decode_raster_bytes(
                            &bytes,
                            request.options.format,
                            &request.cancel,
                        )?;
                        let image = image::RgbaImage::from_raw(
                            decoded.width,
                            decoded.height,
                            decoded.pixels().as_flattened().to_vec(),
                        )
                        .ok_or("Invalid raster preview pixels")?;
                        image::DynamicImage::ImageRgba8(image)
                    };
                    if request.cancel.load(Ordering::Relaxed) {
                        return Err(files::CANCELLED.into());
                    }
                    let thumbnail = decoded
                        .thumbnail(decoded.width().min(512), decoded.height().min(320))
                        .to_rgba8();
                    let mut rgb = Vec::with_capacity(thumbnail.len() / 4 * 3);
                    for (x, y, pixel) in thumbnail.enumerate_pixels() {
                        let checker = if (x / 8 + y / 8) % 2 == 0 { 224 } else { 176 };
                        let alpha = u32::from(pixel[3]);
                        for c in 0..3 {
                            rgb.push(
                                ((u32::from(pixel[c]) * alpha + checker * (255 - alpha) + 127)
                                    / 255) as u8,
                            );
                        }
                    }
                    let image = egui::ColorImage::from_rgb(
                        [thumbnail.width() as usize, thumbnail.height() as usize],
                        &rgb,
                    );
                    Ok(EncodedPreview {
                        bytes: Arc::new(bytes),
                        image,
                    })
                }))
                .unwrap_or_else(|_| Err("Export preview worker failed.".into()));
                if !request.cancel.load(Ordering::Relaxed) {
                    if send.send((request.generation, result)).is_err() {
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
        preview.request(options);
        preview
    }
    pub fn request(&mut self, options: RasterOptions) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.generation += 1;
        self.ready = false;
        let _ = self.requests.send(Request {
            generation: self.generation,
            options,
            cancel: self.cancel.clone(),
        });
    }
    pub fn poll(&mut self) -> Option<Result<EncodedPreview, String>> {
        while let Ok((generation, result)) = self.results.try_recv() {
            if generation == self.generation {
                self.ready = result.is_ok();
                return Some(result);
            }
        }
        None
    }
}
impl Drop for RasterPreview {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paint_rust::files::RasterFormat;
    #[test]
    fn latest_preview_decodes_the_exact_bytes_that_will_be_saved() {
        let ctx = egui::Context::default();
        let document = Document::new(64, 32, [255, 0, 0, 0]).unwrap();
        let mut options = RasterOptions::new(RasterFormat::Bmp);
        let mut preview = RasterPreview::new(ctx, document, options);
        for matte in [[250, 10, 20], [50, 60, 70], [17, 29, 41]] {
            options.matte = matte;
            preview.request(options);
        }
        assert!(!preview.ready);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let result = loop {
            if let Some(result) = preview.poll() {
                break result.unwrap();
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        assert!(preview.ready);
        let decoded = image::load_from_memory(&result.bytes).unwrap().to_rgba8();
        assert!(decoded.pixels().all(|p| p.0 == [17, 29, 41, 255]));
        assert!(
            result
                .image
                .pixels
                .iter()
                .all(|p| *p == egui::Color32::from_rgb(17, 29, 41))
        );
        assert!(result.image.size[0] <= 512 && result.image.size[1] <= 320);
    }
}

#[cfg(test)]
mod jpeg_xl_tests {
    use super::*;
    #[test]
    fn lossless_custom_codec_preview_decodes_the_saved_bytes() {
        let ctx = egui::Context::default();
        let document = Document::new(17, 9, [13, 27, 53, 255]).unwrap();
        let mut preview = RasterPreview::new(
            ctx,
            document,
            RasterOptions::new(files::RasterFormat::JpegXl),
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let result = loop {
            if let Some(result) = preview.poll() {
                break result.unwrap();
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        let decoded = files::decode_raster_bytes(
            &result.bytes,
            files::RasterFormat::JpegXl,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(decoded.pixels(), vec![[13, 27, 53, 255]; 17 * 9]);
        assert!(
            result
                .image
                .pixels
                .iter()
                .all(|pixel| *pixel == egui::Color32::from_rgb(13, 27, 53))
        );
        assert_eq!(result.image.size, [17, 9]);
    }
}
