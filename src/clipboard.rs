use paint_rust::document::{Document, Pixel};
use std::borrow::Cow;

pub trait Backend: Send {
    fn write_text(&mut self, _text: &str) -> Result<(), String> {
        Err("Text clipboard is unavailable".into())
    }
    fn read_text(&mut self) -> Result<String, String> {
        Err("Text clipboard is unavailable".into())
    }
    fn write(&mut self, width: u32, height: u32, pixels: &[Pixel]) -> Result<(), String>;
    fn read(&mut self) -> Result<(u32, u32, Vec<Pixel>), String>;
}

pub struct Clipboard {
    handle: arboard::Clipboard,
}

impl Clipboard {
    pub fn connect() -> Result<Self, String> {
        arboard::Clipboard::new()
            .map(|handle| Self { handle })
            .map_err(|e| e.to_string())
    }
}

impl Backend for Clipboard {
    fn write_text(&mut self, text: &str) -> Result<(), String> {
        self.handle
            .set_text(text)
            .map_err(|error| error.to_string())
    }
    fn read_text(&mut self) -> Result<String, String> {
        self.handle.get_text().map_err(|error| error.to_string())
    }
    fn write(&mut self, width: u32, height: u32, pixels: &[Pixel]) -> Result<(), String> {
        self.handle
            .set_image(arboard::ImageData {
                width: width as usize,
                height: height as usize,
                bytes: Cow::Owned(pixels.iter().flatten().copied().collect()),
            })
            .map_err(|e| e.to_string())
    }
    fn read(&mut self) -> Result<(u32, u32, Vec<Pixel>), String> {
        let image = self.handle.get_image().map_err(|e| e.to_string())?;
        let width = u32::try_from(image.width)
            .map_err(|_| "Clipboard width exceeds the supported range")?;
        let height = u32::try_from(image.height)
            .map_err(|_| "Clipboard height exceeds the supported range")?;
        Document::validate_size(width, height)?;
        if image.bytes.len() != width as usize * height as usize * 4 {
            return Err("Invalid clipboard pixel buffer.".into());
        }
        let pixels = image.bytes.as_chunks::<4>().0.to_vec();
        Ok((width, height, pixels))
    }
}

/// Keep the native clipboard owner alive between background jobs.
pub type Shared = std::sync::Arc<std::sync::Mutex<Option<Box<dyn Backend>>>>;

pub fn with_backend<T>(
    shared: &Shared,
    work: impl FnOnce(&mut dyn Backend) -> Result<T, String>,
) -> Result<T, String> {
    let mut backend = shared
        .lock()
        .map_err(|_| "Clipboard connection lock failed")?;
    if backend.is_none() {
        *backend = Some(Box::new(Clipboard::connect()?));
    }
    work(backend.as_deref_mut().expect("Clipboard connected"))
}
