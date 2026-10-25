//! HEIF primary-image import and HEVC export through the host codec plugins.
use crate::{
    document::{Document, Pixel},
    files::CANCELLED,
};
use libheif_rs::{
    ColorPrimaries, ColorSpace, CompressionFormat, DecodingOptions, EncoderParameterValue,
    EncoderQuality, EncodingOptions, HeifContext, Image, LibHeif, RgbChroma, SecurityLimits,
    TransferCharacteristics,
};
use std::{
    io::{Read, Write},
    sync::atomic::{AtomicBool, Ordering},
};
const INPUT_LIMIT: usize = 128 * 1024 * 1024;
fn check(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}

pub(crate) fn sniff(header: &[u8]) -> Option<crate::files::RasterFormat> {
    if header.get(4..8)? != b"ftyp" {
        return None;
    }
    let size = u32::from_be_bytes(header.get(..4)?.try_into().ok()?);
    let (size, start) = if size == 1 {
        (
            usize::try_from(u64::from_be_bytes(header.get(8..16)?.try_into().ok()?)).ok()?,
            16,
        )
    } else {
        (size as usize, 8)
    };
    let content = header.get(start..size.min(header.len()))?;
    let brands: Vec<_> = content
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 1)
        .map(|(_, brand)| brand)
        .collect();
    if brands
        .iter()
        .any(|brand| matches!(*brand, b"avif" | b"avis"))
    {
        return Some(crate::files::RasterFormat::Avif);
    }
    brands
        .iter()
        .any(|brand| {
            matches!(
                *brand,
                b"heic"
                    | b"heix"
                    | b"heim"
                    | b"heis"
                    | b"hevc"
                    | b"hevx"
                    | b"mif1"
                    | b"mif2"
                    | b"msf1"
            )
        })
        .then_some(crate::files::RasterFormat::Heic)
}

fn limits() -> SecurityLimits {
    let mut limits = SecurityLimits::new();
    // Set every available bound explicitly, even if the host disabled defaults.
    limits.set_max_image_size_pixels(16_777_216);
    limits.set_max_number_of_tiles(256);
    limits.set_max_bayer_pattern_pixels(256);
    limits.set_max_items(1024);
    limits.set_max_color_profile_size(4 * 1024 * 1024);
    limits.set_max_memory_block_size(INPUT_LIMIT as u64);
    limits.set_max_total_memory(256 * 1024 * 1024);
    limits.set_max_components(16);
    limits.set_max_iloc_extents_per_item(1024);
    limits.set_max_size_entity_group(1024);
    limits.set_max_children_per_box(4096);
    limits.set_max_sample_description_box_entries(128);
    limits.set_max_sample_group_description_box_entries(128);
    limits.set_max_sequence_frames(1);
    limits.set_max_number_of_file_brands(64);
    limits.set_max_bad_pixels(1024);
    limits.set_max_iso23001_17_pixel_size_bytes(16);
    limits
}
fn dimensions(width: u32, height: u32) -> Result<(), String> {
    Document::validate_size(width, height)?;
    if width > 16384 || height > 16384 {
        return Err("HEIC dimensions exceed 16,384 pixels per side".into());
    }
    Ok(())
}
fn validate_color(
    profile: Option<&libheif_rs::ColorProfileNCLX>,
    has_icc: bool,
) -> Result<(), String> {
    if let Some(profile) = profile {
        let transfer = profile.transfer_characteristics();
        if matches!(
            transfer,
            TransferCharacteristics::ITU_R_BT_2100_0_PQ
                | TransferCharacteristics::ITU_R_BT_2100_0_HLG
        ) {
            return Err("HDR HEIC requires tone mapping before import".into());
        }
        if !has_icc
            && (!matches!(
                profile.color_primaries(),
                ColorPrimaries::ITU_R_BT_709_5 | ColorPrimaries::Unspecified
            ) || !matches!(
                transfer,
                TransferCharacteristics::IEC_61966_2_1 | TransferCharacteristics::Unspecified
            ))
        {
            return Err("HEIC color encoding requires an ICC profile or conversion to sRGB".into());
        }
    }
    Ok(())
}

pub(crate) fn decode(mut reader: impl Read, cancel: &AtomicBool) -> Result<Document, String> {
    check(cancel)?;
    let mut bytes = Vec::new();
    let mut buffer = [0; 16384];
    loop {
        check(cancel)?;
        let count = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        if bytes.len() + count > INPUT_LIMIT {
            return Err("HEIC input exceeds 128 MiB".into());
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    let heif = LibHeif::new_checked().map_err(|e| e.to_string())?;
    let mut context = HeifContext::new().map_err(|e| e.to_string())?;
    context
        .set_security_limits(&limits())
        .map_err(|e| e.to_string())?;
    context.set_max_decoding_threads(2);
    context.read_bytes(&bytes).map_err(|e| e.to_string())?;
    let handle = context.primary_image_handle().map_err(|e| e.to_string())?;
    dimensions(handle.width(), handle.height())?;
    if handle.is_premultiplied_alpha() {
        return Err("Associated-alpha HEIC is not supported yet".into());
    }
    let profile = handle.color_profile_raw();
    if profile
        .as_ref()
        .is_some_and(|profile| profile.data.len() > 4 * 1024 * 1024)
    {
        return Err("HEIC ICC profile exceeds 4 MiB".into());
    }
    validate_color(handle.color_profile_nclx().as_ref(), profile.is_some())?;
    let mut options = DecodingOptions::new().ok_or("Could not allocate HEIC decoding options")?;
    options.set_strict_decoding(true);
    options.set_convert_hdr_to_8bit(true);
    options.set_output_image_nclx_profile_passthrough(true);
    check(cancel)?;
    let image = heif
        .decode(&handle, ColorSpace::Rgb(RgbChroma::Rgba), Some(options))
        .map_err(|e| format!("HEIC decode failed (a host HEVC decoder plugin is required): {e}"))?;
    check(cancel)?;
    dimensions(image.width(), image.height())?;
    validate_color(image.color_profile_nclx().as_ref(), profile.is_some())?;
    if image.is_premultiplied_alpha() {
        return Err("Associated-alpha HEIC is not supported yet".into());
    }
    let plane = image
        .planes()
        .interleaved
        .ok_or("HEIC RGBA plane is missing")?;
    let (width, height) = (image.width(), image.height());
    let row_bytes = width as usize * 4;
    if (plane.width, plane.height) != (width, height)
        || plane.stride < row_bytes
        || plane.data.len() < plane.stride * height as usize
        || plane.storage_bits_per_pixel != 32
    {
        return Err("Invalid HEIC RGBA plane layout".into());
    }
    let mut pixels = Vec::with_capacity(width as usize * height as usize);
    for row in plane.data.chunks_exact(plane.stride).take(height as usize) {
        check(cancel)?;
        pixels.extend(row[..row_bytes].as_chunks::<4>().0.iter().copied());
    }
    if let Some(profile) = profile {
        crate::color_management::convert_to_srgb(&mut pixels, &profile.data, cancel)?;
    }
    let mut document = Document::from_pixels(width, height, pixels)?;
    if let Some(dpi) = density(&handle) {
        crate::resolution::write(&mut document.metadata, dpi)?;
    }
    Ok(document)
}

// Metadata is optional. Bound both item count and bytes before allocation.
fn density(handle: &libheif_rs::ImageHandle) -> Option<crate::resolution::Dpi> {
    let mut ids = [0; 16];
    let count = handle.metadata_block_ids(&mut ids, b"Exif");
    let mut budget = 4 * 1024 * 1024usize;
    for id in ids.into_iter().take(count) {
        let size = handle.metadata_size(id);
        if size > budget {
            continue;
        }
        budget -= size;
        let Ok(data) = handle.metadata(id) else {
            continue;
        };
        // HEIF EXIF starts with a big-endian offset relative to byte four.
        let Some(offset) = data
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_be_bytes)
        else {
            continue;
        };
        let Some(start) = (offset as usize).checked_add(4) else {
            continue;
        };
        if let Some(dpi) = data.get(start..).and_then(crate::raster_density::read_exif) {
            return Some(dpi);
        }
    }
    None
}

pub(crate) fn encode(
    pixels: &[Pixel],
    size: (u32, u32),
    mut writer: impl Write,
    quality: u8,
    dpi: crate::resolution::Dpi,
    cancel: &AtomicBool,
) -> Result<(), String> {
    check(cancel)?;
    dimensions(size.0, size.1)?;
    let heif = LibHeif::new_checked().map_err(|e| e.to_string())?;
    let mut context = HeifContext::new().map_err(|e| e.to_string())?;
    context
        .set_security_limits(&limits())
        .map_err(|e| e.to_string())?;
    let mut encoder = heif
        .encoder_for_format(CompressionFormat::Hevc)
        .map_err(|e| format!("HEIC export requires a host HEVC encoder plugin: {e}"))?;
    encoder
        .set_quality(EncoderQuality::Lossy(quality))
        .map_err(|e| e.to_string())?;
    // These pass-through controls are specific to the common x265 backend.
    if encoder.name().to_ascii_lowercase().contains("x265") {
        for (name, value) in [
            ("preset", "fast"),
            ("chroma", "444"),
            ("x265:pools", "2"),
            ("x265:frame-threads", "1"),
            ("x265:log-level", "error"),
            ("x265:info", "0"),
        ] {
            encoder
                .set_parameter_value(name, EncoderParameterValue::String(value.into()))
                .map_err(|e| e.to_string())?;
        }
    }
    let mut image =
        Image::new(size.0, size.1, ColorSpace::Rgb(RgbChroma::Rgba)).map_err(|e| e.to_string())?;
    image
        .create_plane(libheif_rs::Channel::Interleaved, size.0, size.1, 8)
        .map_err(|e| e.to_string())?;
    let plane = image
        .planes_mut()
        .interleaved
        .ok_or("HEIC RGBA plane allocation failed")?;
    plane.data.fill(0);
    for (source, row) in pixels
        .chunks_exact(size.0 as usize)
        .zip(plane.data.chunks_exact_mut(plane.stride))
    {
        check(cancel)?;
        row[..size.0 as usize * 4].copy_from_slice(source.as_flattened());
    }
    image
        .set_color_profile_raw(&libheif_rs::ColorProfileRaw::new(
            libheif_rs::color_profile_types::PROF,
            crate::color_management::srgb_profile()?,
        ))
        .map_err(|e| e.to_string())?;
    let mut options = EncodingOptions::new().map_err(|e| e.to_string())?;
    options.set_save_alpha_channel(true);
    let handle = context
        .encode_image(&image, &mut encoder, Some(options))
        .map_err(|e| e.to_string())?;
    check(cancel)?;
    context
        .add_exif_metadata(&handle, &crate::raster_density::exif(dpi))
        .map_err(|e| e.to_string())?;
    // The wrapper's vector writer replaces earlier chunks. Use its file writer
    // and a private temporary file to retain all container chunks correctly.
    let temporary = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    context
        .write_to_file(
            temporary
                .path()
                .to_str()
                .ok_or("HEIC temporary path is not UTF-8")?,
        )
        .map_err(|e| e.to_string())?;
    if temporary
        .as_file()
        .metadata()
        .map_err(|e| e.to_string())?
        .len()
        > INPUT_LIMIT as u64
    {
        return Err("Encoded HEIC exceeds 128 MiB".into());
    }
    let mut file = std::fs::File::open(temporary.path()).map_err(|e| e.to_string())?;
    let mut buffer = [0; 65536];
    loop {
        check(cancel)?;
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        writer
            .write_all(&buffer[..count])
            .map_err(|e| e.to_string())?;
    }
    check(cancel)
}
