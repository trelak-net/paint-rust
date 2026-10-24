use paint_rust::document::{Document, Pixel};
use std::borrow::Cow;

pub trait Backend {
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
