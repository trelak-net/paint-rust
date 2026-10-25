//! Bounded still-image AVIF import into the straight RGBA8 sRGB workspace.
use crate::{document::Document, files::CANCELLED, transform::Orientation};
use dav1d::pixel::{ColorPrimaries, MatrixCoefficients, TransferCharacteristic, YUVRange};
use std::{
    io::{Cursor, Read},
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

// The image adapter eagerly allocates native frames before its ImageDecoder
// limits can be applied. Drain each item first with a native frame-size limit.
// Reject sequences rather than allowing a later oversized frame to escape checks.
fn preflight(data: &[u8], cancel: &AtomicBool) -> Result<dav1d::Picture, String> {
    let mut settings = dav1d::Settings::new();
    settings.set_n_threads(1);
    settings.set_max_frame_delay(1);
    settings.set_frame_size_limit(16_777_216);
    let mut decoder = dav1d::Decoder::with_settings(&settings).map_err(|e| e.to_string())?;
    let mut pending = match decoder.send_data(data.to_vec(), None, None, None) {
        Ok(()) => false,
        Err(dav1d::Error::Again) => true,
        Err(e) => return Err(e.to_string()),
    };
    let mut picture = None;
    for _ in 0..32 {
        check(cancel)?;
        match decoder.get_picture() {
            Ok(frame) => {
                Document::validate_size(frame.width(), frame.height())?;
                if frame.width() > 16384 || frame.height() > 16384 {
                    return Err("AVIF dimensions exceed 16,384 pixels per side".into());
                }
                if picture.replace(frame).is_some() {
                    return Err("Multiple AV1 frames per AVIF item are not supported yet".into());
                }
            }
            Err(dav1d::Error::Again) if !pending => {
                return picture.ok_or_else(|| "Incomplete AVIF frame".into());
            }
            Err(dav1d::Error::Again) => (),
            Err(e) => return Err(e.to_string()),
        }
        pending = match decoder.send_pending_data() {
            Ok(()) => false,
            Err(dav1d::Error::Again) => true,
            Err(e) => return Err(e.to_string()),
        };
    }
    Err("AVIF decoder made no bounded progress".into())
}

// Validate property color declarations as well as the coded AV1 header. The
// adapter uses AV1 color information; reject container overrides it cannot apply.
// All properties are checked conservatively, including unused item properties.
fn properties(
    bytes: &[u8],
    profile: bool,
    picture: &dav1d::Picture,
    depth: usize,
) -> Result<(), String> {
    if depth > 3 {
        return Err("Invalid AVIF box nesting".into());
    }
    let mut remaining = bytes;
    while !remaining.is_empty() {
        let header = remaining.get(..8).ok_or("Truncated AVIF box")?;
        let size = u32::from_be_bytes(header[..4].try_into().unwrap());
        let (size, start) = match size {
            0 => (remaining.len(), 8),
            1 => (
                usize::try_from(u64::from_be_bytes(
                    remaining
                        .get(8..16)
                        .ok_or("Truncated AVIF box")?
                        .try_into()
                        .unwrap(),
                ))
                .map_err(|_| "AVIF box is too large")?,
                16,
            ),
            size => (size as usize, 8),
        };
        let payload = remaining.get(start..size).ok_or("Invalid AVIF box size")?;
        match (&header[4..8], depth) {
            (b"meta", 0) => properties(
                payload.get(4..).ok_or("Truncated AVIF metadata")?,
                profile,
                picture,
                1,
            )?,
            (b"iprp", 1) | (b"ipco", 2) => properties(payload, profile, picture, depth + 1)?,
            (b"colr", 3) if payload.starts_with(b"nclx") => {
                let fields = payload
                    .get(4..11)
                    .ok_or("Truncated AVIF color declaration")?;
                let primaries = u16::from_be_bytes([fields[0], fields[1]]);
                let transfer = u16::from_be_bytes([fields[2], fields[3]]);
                let matrix = u16::from_be_bytes([fields[4], fields[5]]);
                let matches_matrix = match matrix {
                    0 => picture.matrix_coefficients() == MatrixCoefficients::Identity,
                    1 => picture.matrix_coefficients() == MatrixCoefficients::BT709,
                    2 => true,
                    5 | 6 => picture.matrix_coefficients() == MatrixCoefficients::BT470BG,
                    8 => picture.matrix_coefficients() == MatrixCoefficients::YCgCo,
                    9 => {
                        picture.matrix_coefficients()
                            == MatrixCoefficients::BT2020NonConstantLuminance
                    }
                    _ => false,
                };
                if !matches_matrix
                    || (fields[6] & 128 != 0) != (picture.color_range() == YUVRange::Full)
                {
                    return Err("AVIF container color overrides are not supported yet".into());
                }
                if matches!(transfer, 16 | 18) {
                    return Err("HDR AVIF requires tone mapping before import".into());
                }
                if !profile && (!matches!(primaries, 1 | 2) || !matches!(transfer, 2 | 13)) {
                    return Err(
                        "AVIF color encoding requires an ICC profile or conversion to sRGB".into(),
                    );
                }
            }
            (b"pasp", 3) if payload.len() != 8 || payload[..4] != payload[4..8] => {
                return Err("Non-square AVIF pixels are not supported yet".into());
            }
            _ => (),
        }
        remaining = &remaining[size..];
    }
    Ok(())
}

pub(crate) fn decode(mut reader: impl Read, cancel: &AtomicBool) -> Result<Document, String> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 16384];
    loop {
        check(cancel)?;
        let count = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        if bytes.len() + count > INPUT_LIMIT {
            return Err("AVIF input exceeds 128 MiB".into());
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    let context = mp4parse::read_avif(&mut Cursor::new(&bytes), mp4parse::ParseStrictness::Normal)
        .map_err(|e| e.to_string())?;
    if context.sequence.is_some() || !context.unsupported_features.is_empty() {
        return Err(
            "AVIF sequences, grids, cropping and layered items are not supported yet".into(),
        );
    }
    if context.premultiplied_alpha {
        return Err("Associated-alpha AVIF is not supported yet".into());
    }
    let profile = context
        .icc_colour_information()
        .transpose()
        .map_err(|e| e.to_string())?;
    if profile.is_some_and(|icc| icc.len() > 4 * 1024 * 1024) {
        return Err("AVIF ICC profile exceeds 4 MiB".into());
    }
    let primary = preflight(
        context
            .primary_item_coded_data()
            .ok_or("AVIF primary image is missing")?,
        cancel,
    )?;
    if matches!(
        primary.transfer_characteristic(),
        TransferCharacteristic::PerceptualQuantizer | TransferCharacteristic::HybridLogGamma
    ) {
        return Err("HDR AVIF requires tone mapping before import".into());
    }
    if profile.is_none()
        && (!matches!(
            primary.color_primaries(),
            ColorPrimaries::BT709 | ColorPrimaries::Unspecified
        ) || !matches!(
            primary.transfer_characteristic(),
            TransferCharacteristic::SRGB | TransferCharacteristic::Unspecified
        ))
    {
        return Err("AVIF color encoding requires an ICC profile or conversion to sRGB".into());
    }
    if let Some(alpha) = context.alpha_item_coded_data() {
        let alpha = preflight(alpha, cancel)?;
        if (alpha.width(), alpha.height()) != (primary.width(), primary.height()) {
            return Err("AVIF alpha dimensions do not match the primary image".into());
        }
    }
    properties(&bytes, profile.is_some(), &primary, 0)?;
    drop(primary);
    check(cancel)?;
    let decoder =
        image::codecs::avif::AvifDecoder::new(Cursor::new(&bytes)).map_err(|e| e.to_string())?;
    let decoded = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let (mut width, mut height) = decoded.dimensions();
    let mut pixels = Vec::with_capacity(width as usize * height as usize);
    for row in decoded.rows() {
        check(cancel)?;
        pixels.extend(row.map(|pixel| pixel.0));
    }
    drop(decoded);
    if let Some(profile) = profile {
        crate::color_management::convert_to_srgb(&mut pixels, profile, cancel)?;
    }
    let mut operations = Vec::new();
    use mp4parse::ImageRotation;
    match context.image_rotation().map_err(|e| e.to_string())? {
        ImageRotation::D0 => (),
        ImageRotation::D90 => operations.push(Orientation::Counterclockwise),
        ImageRotation::D180 => operations.push(Orientation::HalfTurn),
        ImageRotation::D270 => operations.push(Orientation::Clockwise),
    }
    let mirror = context.image_mirror_ptr().map_err(|e| e.to_string())?;
    // SAFETY: this optional pointer borrows a property owned by the live,
    // immutable context. Copy only its meaning; never retain the reference.
    if let Some(mirror) = unsafe { mirror.as_ref() } {
        operations.push(match mirror {
            mp4parse::ImageMirror::TopBottom => Orientation::FlipVertical,
            mp4parse::ImageMirror::LeftRight => Orientation::FlipHorizontal,
        });
    }
    for operation in operations {
        pixels = crate::transform::orient_cancellable(&pixels, width, height, operation, cancel)
            .ok_or(CANCELLED)?;
        if matches!(
            operation,
            Orientation::Clockwise | Orientation::Counterclockwise
        ) {
            std::mem::swap(&mut width, &mut height);
        }
    }
    Document::from_pixels(width, height, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_drains_and_rejects_additional_coded_frames() {
        let bytes = include_bytes!("../tests/fixtures/avif/rgb-lossless.avif");
        let context =
            mp4parse::read_avif(&mut Cursor::new(bytes), mp4parse::ParseStrictness::Normal)
                .unwrap();
        let coded = context.primary_item_coded_data().unwrap();
        let mut sequence = coded.to_vec();
        sequence.extend_from_slice(coded);
        let error = preflight(&sequence, &AtomicBool::new(false)).err().unwrap();
        assert!(error.contains("Multiple AV1 frames"), "{error}");
    }
}
