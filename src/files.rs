use crate::document::{BlendMode, Document, Layer};
use image::{ImageEncoder, ImageReader, codecs::png::PngEncoder};
use std::{
    fs::File,
    io::{BufReader, BufWriter, Cursor, Read, Write},
    path::Path,
};

const PROJECT_MAGIC: &[u8; 8] = b"PRUST\0\x02\0";

pub fn open(path: &Path) -> Result<Document, String> {
    let mut header = [0; 8];
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let read = file.read(&mut header).map_err(|e| e.to_string())?;
    if (read >= 5 && &header[..5] == b"PRUST")
        || path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("prust"))
    {
        return open_project(path);
    }
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

fn write_atomic(
    path: &Path,
    write: impl FnOnce(&mut File) -> Result<(), String>,
) -> Result<(), String> {
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
    write(temp.as_file_mut())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.error.to_string())?;
    Ok(())
}

fn encode_png(
    pixels: &[[u8; 4]],
    width: u32,
    height: u32,
    writer: impl Write,
) -> Result<(), String> {
    let bytes: Vec<u8> = pixels.iter().flatten().copied().collect();
    PngEncoder::new(writer)
        .write_image(&bytes, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())
}

pub fn save_png(document: &Document, path: &Path) -> Result<(), String> {
    write_atomic(path, |file| {
        let mut writer = BufWriter::new(file);
        encode_png(
            &document.composite(),
            document.width,
            document.height,
            &mut writer,
        )?;
        writer.flush().map_err(|e| e.to_string())
    })
}

pub fn save_project(document: &Document, path: &Path) -> Result<(), String> {
    write_atomic(path, |file| {
        let mut writer = BufWriter::new(file);
        writer.write_all(PROJECT_MAGIC).map_err(|e| e.to_string())?;
        for value in [
            document.width,
            document.height,
            document.layers().len() as u32,
            document.active_layer() as u32,
        ] {
            writer
                .write_all(&value.to_le_bytes())
                .map_err(|e| e.to_string())?;
        }
        for layer in document.layers() {
            writer
                .write_all(&(layer.name.len() as u32).to_le_bytes())
                .map_err(|e| e.to_string())?;
            writer
                .write_all(layer.name.as_bytes())
                .map_err(|e| e.to_string())?;
            writer
                .write_all(&[
                    u8::from(layer.visible),
                    layer.opacity,
                    layer.blend_mode as u8,
                ])
                .map_err(|e| e.to_string())?;
            let mut png = Vec::new();
            encode_png(layer.pixels(), document.width, document.height, &mut png)?;
            writer
                .write_all(&(png.len() as u64).to_le_bytes())
                .map_err(|e| e.to_string())?;
            writer.write_all(&png).map_err(|e| e.to_string())?;
        }
        writer.flush().map_err(|e| e.to_string())
    })
}

fn read_array<const N: usize>(reader: &mut impl Read) -> Result<[u8; N], String> {
    let mut bytes = [0; N];
    reader.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

pub fn open_project(path: &Path) -> Result<Document, String> {
    let mut reader = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
    let magic = read_array::<8>(&mut reader)?;
    if magic[..6] != PROJECT_MAGIC[..6] || magic[7] != 0 || !matches!(magic[6], 1 | 2) {
        return Err("Unsupported project signature or version.".into());
    }
    let width = u32::from_le_bytes(read_array(&mut reader)?);
    let height = u32::from_le_bytes(read_array(&mut reader)?);
    let count = u32::from_le_bytes(read_array(&mut reader)?) as usize;
    let active = u32::from_le_bytes(read_array(&mut reader)?) as usize;
    Document::validate_size(width, height)?;
    if count == 0
        || count > 32
        || active >= count
        || count as u64 * u64::from(width) * u64::from(height) * 4 > 256 * 1024 * 1024
    {
        return Err("Invalid layer count, active layer or document memory limit.".into());
    }
    let mut layers = Vec::new();
    for _ in 0..count {
        let length = u32::from_le_bytes(read_array(&mut reader)?) as usize;
        if length > 4096 {
            return Err("Layer name is too long.".into());
        }
        let mut name = vec![0; length];
        reader.read_exact(&mut name).map_err(|e| e.to_string())?;
        let name = String::from_utf8(name).map_err(|e| e.to_string())?;
        let flags = read_array::<2>(&mut reader)?;
        if flags[0] > 1 {
            return Err("Invalid layer visibility.".into());
        }
        let mode = if magic[6] >= 2 {
            let index = read_array::<1>(&mut reader)?[0] as usize;
            *BlendMode::ALL
                .get(index)
                .ok_or("Unsupported layer blend mode")?
        } else {
            BlendMode::Normal
        };
        let size = u64::from_le_bytes(read_array(&mut reader)?);
        if size > 128 * 1024 * 1024 {
            return Err("Layer payload exceeds the memory limit.".into());
        }
        let mut png = Vec::new();
        (&mut reader)
            .take(size)
            .read_to_end(&mut png)
            .map_err(|e| e.to_string())?;
        if png.len() as u64 != size {
            return Err("Truncated layer payload.".into());
        }
        let dimensions = ImageReader::with_format(Cursor::new(&png), image::ImageFormat::Png)
            .into_dimensions()
            .map_err(|e| e.to_string())?;
        if dimensions != (width, height) {
            return Err("Layer dimensions do not match the document.".into());
        }
        let mut image_reader = ImageReader::with_format(Cursor::new(png), image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(128 * 1024 * 1024);
        image_reader.limits(limits);
        let image = image_reader.decode().map_err(|e| e.to_string())?.to_rgba8();
        let mut layer = Layer::new(name, image.pixels().map(|p| p.0).collect());
        layer.visible = flags[0] != 0;
        layer.opacity = flags[1];
        layer.blend_mode = mode;
        layers.push(layer);
    }
    if reader.read(&mut [0; 1]).map_err(|e| e.to_string())? != 0 {
        return Err("Unexpected data after the project.".into());
    }
    Document::from_layers(width, height, layers, active)
}
