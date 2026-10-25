//! JPEG XL first-frame import into the editor's straight RGBA8 sRGB workspace.
use crate::{document::Document, files::CANCELLED};
use jxl_oxide::{AllocTracker, EnumColourEncoding, InitializeResult, JxlImage, RenderingIntent};
use std::{
    io::{Read, Write},
    sync::atomic::{AtomicBool, Ordering},
};

const INPUT_LIMIT: usize = 256 * 1024 * 1024;
fn check(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}

pub(crate) fn decode(mut reader: impl Read, cancel: &AtomicBool) -> Result<Document, String> {
    check(cancel)?;
    let mut buffer = [0; 4096];
    let (mut buffered, mut total) = (0, 0);
    let mut uninit = JxlImage::builder()
        .alloc_tracker(AllocTracker::with_limit(INPUT_LIMIT))
        .build_uninit();
    let mut feed = |buffer: &mut [u8; 4096], buffered: &mut usize| -> Result<bool, String> {
        check(cancel)?;
        if *buffered == buffer.len() {
            return Err("JPEG XL decoder made no progress".into());
        }
        let count = reader
            .read(&mut buffer[*buffered..])
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Ok(false);
        }
        total += count;
        if total > INPUT_LIMIT {
            return Err("JPEG XL input exceeds 256 MiB".into());
        }
        *buffered += count;
        Ok(true)
    };
    let mut image = loop {
        if !feed(&mut buffer, &mut buffered)? {
            return Err("Incomplete JPEG XL frame".into());
        }
        let consumed = uninit
            .feed_bytes(&buffer[..buffered])
            .map_err(|e| e.to_string())?;
        buffer.copy_within(consumed..buffered, 0);
        buffered -= consumed;
        match uninit.try_init().map_err(|e| e.to_string())? {
            InitializeResult::Initialized(image) => break image,
            InitializeResult::NeedMoreData(next) => uninit = next,
        }
    };
    Document::validate_size(image.width(), image.height())?;
    if image.width() > 16384 || image.height() > 16384 {
        return Err("JPEG XL dimensions exceed 16,384 pixels per side".into());
    }
    if image
        .original_icc()
        .is_some_and(|profile| profile.len() > 4 * 1024 * 1024)
    {
        return Err("JPEG XL ICC profile exceeds 4 MiB".into());
    }
    if image.hdr_type().is_some() {
        return Err("HDR JPEG XL requires tone mapping before import".into());
    }
    if image
        .image_header()
        .metadata
        .ec_info
        .iter()
        .any(|channel| channel.alpha_associated() == Some(true))
    {
        return Err("Associated-alpha JPEG XL is not supported yet".into());
    }
    image.request_color_encoding(if image.pixel_format().is_grayscale() {
        EnumColourEncoding::gray_srgb(RenderingIntent::Relative)
    } else {
        EnumColourEncoding::srgb(RenderingIntent::Relative)
    });
    while image.num_loaded_keyframes() == 0 {
        if !feed(&mut buffer, &mut buffered)? {
            return Err("Incomplete JPEG XL frame".into());
        }
        let consumed = image
            .feed_bytes(&buffer[..buffered])
            .map_err(|e| e.to_string())?;
        buffer.copy_within(consumed..buffered, 0);
        buffered -= consumed;
    }
    // Still-image metadata can follow the codestream. Animated input deliberately
    // stops at the first displayed frame rather than decoding the full animation.
    if image.image_header().metadata.animation.is_none() {
        while feed(&mut buffer, &mut buffered)? {
            let consumed = image
                .feed_bytes(&buffer[..buffered])
                .map_err(|e| e.to_string())?;
            buffer.copy_within(consumed..buffered, 0);
            buffered -= consumed;
        }
        image.finalize().map_err(|e| e.to_string())?;
    }
    let mut density = match image.aux_boxes().first_exif() {
        Ok(jxl_oxide::AuxBoxData::Data(exif)) => exif
            .payload()
            .get(exif.tiff_header_offset() as usize..)
            .and_then(crate::raster_density::read_exif),
        _ => None,
    };
    if image.image_header().metadata.orientation >= 5
        && let Some(dpi) = &mut density
    {
        std::mem::swap(&mut dpi.x, &mut dpi.y);
    }
    check(cancel)?;
    let rendered = image.render_frame(0).map_err(|e| e.to_string())?;
    check(cancel)?;
    let mut stream = rendered.stream();
    let (width, height, channels) = (stream.width(), stream.height(), stream.channels() as usize);
    Document::validate_size(width, height)?;
    if !(1..=4).contains(&channels) {
        return Err("Unsupported JPEG XL channel layout".into());
    }
    let mut row = vec![0_u8; width as usize * channels];
    let mut pixels = Vec::with_capacity(width as usize * height as usize);
    for _ in 0..height {
        check(cancel)?;
        if stream.write_to_buffer(&mut row) != row.len() {
            return Err("Incomplete JPEG XL pixels".into());
        }
        for pixel in row.chunks(channels) {
            pixels.push(match channels {
                1 => [pixel[0], pixel[0], pixel[0], 255],
                2 => [pixel[0], pixel[0], pixel[0], pixel[1]],
                3 => [pixel[0], pixel[1], pixel[2], 255],
                _ => [pixel[0], pixel[1], pixel[2], pixel[3]],
            });
        }
    }
    let mut document = Document::from_pixels(width, height, pixels)?;
    if let Some(dpi) = density {
        crate::resolution::write(&mut document.metadata, dpi)?;
    }
    Ok(document)
}

/// Wrap the lossless codestream with only application-owned density metadata.
pub(crate) fn write_container(
    codestream: &[u8],
    dpi: crate::resolution::Dpi,
    mut writer: impl Write,
) -> Result<(), String> {
    let exif = crate::raster_density::exif(dpi);
    writer
        .write_all(b"\0\0\0\x0cJXL \r\n\x87\n")
        .map_err(|e| e.to_string())?;
    writer
        .write_all(b"\0\0\0\x14ftypjxl \0\0\0\0jxl ")
        .map_err(|e| e.to_string())?;
    writer
        .write_all(&((exif.len() + 12) as u32).to_be_bytes())
        .map_err(|e| e.to_string())?;
    writer
        .write_all(b"Exif\0\0\0\0")
        .map_err(|e| e.to_string())?;
    writer.write_all(&exif).map_err(|e| e.to_string())?;
    let size =
        u32::try_from(codestream.len() + 8).map_err(|_| "JPEG XL codestream is too large")?;
    writer
        .write_all(&size.to_be_bytes())
        .map_err(|e| e.to_string())?;
    writer.write_all(b"jxlc").map_err(|e| e.to_string())?;
    writer.write_all(codestream).map_err(|e| e.to_string())
}
