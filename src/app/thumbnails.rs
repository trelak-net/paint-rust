use super::*;
use paint_rust::document::Pixel;
use std::sync::{Arc, Weak};

const SIDE: usize = 40;

struct Entry {
    pixels: Weak<Vec<Pixel>>,
    dimensions: (u32, u32),
    texture: TextureHandle,
}

#[derive(Default)]
pub(super) struct LayerThumbnails {
    entries: Vec<Entry>,
}

impl LayerThumbnails {
    pub(super) fn texture(&self, index: usize) -> egui::TextureId {
        self.entries[index].texture.id()
    }

    pub(super) fn update(&mut self, ctx: &egui::Context, document: &Document) {
        let dimensions = (document.width, document.height);
        let mut next: Vec<Entry> = Vec::with_capacity(document.layers().len());
        for layer in document.layers() {
            let pixels = layer.shared_pixels();
            let key = Arc::downgrade(&pixels);
            let existing = self
                .entries
                .iter()
                .chain(next.iter())
                .find(|entry| entry.dimensions == dimensions && entry.pixels.ptr_eq(&key));
            let texture = existing
                .map(|entry| entry.texture.clone())
                .unwrap_or_else(|| {
                    ctx.load_texture(
                        "layer-thumbnail",
                        thumbnail(&pixels, dimensions),
                        TextureOptions::LINEAR,
                    )
                });
            next.push(Entry {
                pixels: key,
                dimensions,
                texture,
            });
        }
        // Weak keys avoid retaining old full-resolution buffers after edits or tab changes.
        self.entries = next;
    }
}

fn thumbnail(pixels: &[Pixel], (width, height): (u32, u32)) -> egui::ColorImage {
    let scale = (SIDE as f64 / width as f64).min(SIDE as f64 / height as f64);
    let w = (width as f64 * scale).round().max(1.0) as usize;
    let h = (height as f64 * scale).round().max(1.0) as usize;
    let left = (SIDE - w) / 2;
    let top = (SIDE - h) / 2;
    let mut image = egui::ColorImage::filled([SIDE, SIDE], Color32::TRANSPARENT);
    for y in 0..h {
        for x in 0..w {
            let checker = if (x / 5 + y / 5) % 2 == 0 {
                224_u32
            } else {
                176
            };
            let mut rgb = [0_u32; 3];
            // Fixed-size sampling bounds work regardless of the canvas dimensions.
            for sy in 0..4 {
                for sx in 0..4 {
                    let ix = ((x * 4 + sx) as u64 * width as u64 / (w * 4) as u64) as usize;
                    let iy = ((y * 4 + sy) as u64 * height as u64 / (h * 4) as u64) as usize;
                    let pixel = pixels[iy * width as usize + ix];
                    for channel in 0..3 {
                        rgb[channel] += pixel[channel] as u32 * pixel[3] as u32
                            + checker * (255 - pixel[3] as u32);
                    }
                }
            }
            let rgb = rgb.map(|v| ((v + 2040) / 4080) as u8);
            image.pixels[(y + top) * SIDE + x + left] = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
        }
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnails_fit_extreme_aspects_and_show_transparency_without_hidden_rgb() {
        let image = thumbnail(&vec![[255, 0, 0, 255]; 4096], (4096, 1));
        assert_eq!(
            image.pixels.iter().filter(|p| **p == Color32::RED).count(),
            SIDE
        );
        assert_eq!(
            image
                .pixels
                .iter()
                .filter(|p| **p == Color32::TRANSPARENT)
                .count(),
            SIDE * (SIDE - 1)
        );
        let transparent = thumbnail(&[[255, 0, 90, 0]], (1, 1));
        assert_eq!(transparent.pixels[0], Color32::from_gray(224));
        assert_eq!(transparent.pixels[5], Color32::from_gray(176));
    }

    #[test]
    fn cache_tracks_shared_pixels_without_retaining_full_size_buffers() {
        let ctx = egui::Context::default();
        let mut doc = Document::new(4, 4, [255; 4]).unwrap();
        let mut cache = LayerThumbnails::default();
        cache.update(&ctx, &doc);
        let first = cache.texture(0);
        doc.add_layer(true);
        cache.update(&ctx, &doc);
        assert_eq!(cache.texture(0), first);
        assert_eq!(cache.texture(1), first);
        doc.set_layer_properties("Renamed".into(), 100);
        cache.update(&ctx, &doc);
        assert_eq!(cache.texture(1), first);
        doc.invert();
        cache.update(&ctx, &doc);
        assert_ne!(cache.texture(1), first);
        doc.move_layer(false);
        cache.update(&ctx, &doc);
        assert_eq!(cache.texture(1), first);
        let pixels = Arc::downgrade(&doc.shared_pixels());
        drop(doc);
        assert!(pixels.upgrade().is_none());
    }
}
