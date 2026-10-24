use crate::document::Document;
use image::{ImageEncoder, ImageReader, codecs::png::PngEncoder};
use std::{
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::Path,
};

pub fn open(path: &Path) -> Result<Document, String> {
    let dimensions = ImageReader::new(BufReader::new(File::open(path).map_err(|e| e.to_string())?))
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .into_dimensions()
        .map_err(|e| e.to_string())?;
    Document::validate_size(dimensions.0, dimensions.1)?;
    let mut reader = ImageReader::new(BufReader::new(File::open(path).map_err(|e| e.to_string())?))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| e.to_string())?.to_rgba8();
    Document::from_pixels(
        image.width(),
        image.height(),
        image.pixels().map(|p| p.0).collect(),
    )
}

pub fn save_png(document: &Document, path: &Path) -> Result<(), String> {
    if let Ok(metadata) = std::fs::metadata(path)
        && metadata.permissions().readonly()
    {
        return Err("The destination is read-only.".into());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    {
        let mut writer = BufWriter::new(temp.as_file_mut());
        let bytes: Vec<u8> = document.pixels().iter().flatten().copied().collect();
        PngEncoder::new(&mut writer)
            .write_image(
                &bytes,
                document.width,
                document.height,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
    }
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.error.to_string())?;
    Ok(())
}
