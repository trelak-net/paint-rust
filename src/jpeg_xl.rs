//! JPEG XL first-frame import into the editor's straight RGBA8 sRGB workspace.
use crate::{document::Document, files::CANCELLED};
use jxl_oxide::{AllocTracker, EnumColourEncoding, InitializeResult, JxlImage, RenderingIntent};
use std::{
    io::Read,
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
    let mut feed = |buffer: &mut [u8; 4096], buffered: &mut usize| -> Result<(), String> {
        check(cancel)?;
        if *buffered == buffer.len() {
            return Err("JPEG XL decoder made no progress".into());
        }
        let count = reader
            .read(&mut buffer[*buffered..])
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("Incomplete JPEG XL frame".into());
        }
        total += count;
        if total > INPUT_LIMIT {
            return Err("JPEG XL input exceeds 256 MiB".into());
        }
        *buffered += count;
        Ok(())
    };
    let mut image = loop {
        feed(&mut buffer, &mut buffered)?;
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
        feed(&mut buffer, &mut buffered)?;
        let consumed = image
            .feed_bytes(&buffer[..buffered])
            .map_err(|e| e.to_string())?;
        buffer.copy_within(consumed..buffered, 0);
        buffered -= consumed;
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
    Document::from_pixels(width, height, pixels)
}
