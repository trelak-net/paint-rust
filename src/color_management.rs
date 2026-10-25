use crate::document::Pixel;
use lcms2::{ColorSpaceSignature, Intent, PixelFormat, Profile, Transform};
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

pub fn srgb_profile() -> Result<Vec<u8>, String> {
    static PROFILE: OnceLock<Result<Vec<u8>, String>> = OnceLock::new();
    PROFILE
        .get_or_init(|| Profile::new_srgb().icc().map_err(|e| e.to_string()))
        .clone()
}

pub fn convert_to_srgb(
    pixels: &mut [Pixel],
    profile: &[u8],
    cancel: &AtomicBool,
) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Err(crate::files::CANCELLED.into());
    }
    if profile.len() > 4 * 1024 * 1024 {
        return Err("ICC profile exceeds 4 MiB".into());
    }
    let source = Profile::new_icc(profile).map_err(|e| format!("Invalid ICC profile: {e}"))?;
    let target = Profile::new_srgb();
    match source.color_space() {
        ColorSpaceSignature::RgbData => {
            let transform: Transform<[u8; 3], [u8; 3]> = Transform::new(
                &source,
                PixelFormat::RGB_8,
                &target,
                PixelFormat::RGB_8,
                Intent::RelativeColorimetric,
            )
            .map_err(|e| format!("Unsupported ICC transform: {e}"))?;
            for chunk in pixels.chunks_mut(4096) {
                if cancel.load(Ordering::Relaxed) {
                    return Err(crate::files::CANCELLED.into());
                }
                let mut rgb: Vec<_> = chunk.iter().map(|p| [p[0], p[1], p[2]]).collect();
                transform.transform_in_place(&mut rgb);
                for (pixel, rgb) in chunk.iter_mut().zip(rgb) {
                    pixel[..3].copy_from_slice(&rgb);
                }
            }
        }
        ColorSpaceSignature::GrayData => {
            let transform: Transform<u8, [u8; 3]> = Transform::new(
                &source,
                PixelFormat::GRAY_8,
                &target,
                PixelFormat::RGB_8,
                Intent::RelativeColorimetric,
            )
            .map_err(|e| format!("Unsupported ICC transform: {e}"))?;
            for chunk in pixels.chunks_mut(4096) {
                if cancel.load(Ordering::Relaxed) {
                    return Err(crate::files::CANCELLED.into());
                }
                if chunk.iter().any(|p| p[0] != p[1] || p[1] != p[2]) {
                    return Err("Grayscale ICC profile does not match decoded image".into());
                }
                let gray: Vec<_> = chunk.iter().map(|p| p[0]).collect();
                let mut rgb = vec![[0; 3]; chunk.len()];
                transform.transform_pixels(&gray, &mut rgb);
                for (pixel, rgb) in chunk.iter_mut().zip(rgb) {
                    pixel[..3].copy_from_slice(&rgb);
                }
            }
        }
        _ => {
            return Err(
                "Unsupported ICC color space; convert this image to RGB before opening it".into(),
            );
        }
    }
    Ok(())
}
