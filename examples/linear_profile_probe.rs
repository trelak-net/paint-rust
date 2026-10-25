//! Generate an original linear-RGB PNG for codec color-management comparisons.
use image::ImageEncoder;
use lcms2::{CIExyY, CIExyYTRIPLE, Profile, ToneCurve};
fn main() -> Result<(), String> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("Usage: linear_profile_probe NEW_OUTPUT.png")?;
    let writer = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let xy = |x, y| CIExyY { x, y, Y: 1.0 };
    let curve = ToneCurve::new(1.0);
    let profile = Profile::new_rgb(
        &xy(0.3127, 0.3290),
        &CIExyYTRIPLE {
            Red: xy(0.64, 0.33),
            Green: xy(0.30, 0.60),
            Blue: xy(0.15, 0.06),
        },
        &[&curve, &curve, &curve],
    )
    .map_err(|e| e.to_string())?;
    let mut encoder = image::codecs::png::PngEncoder::new(writer);
    encoder
        .set_icc_profile(profile.icc().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    encoder
        .write_image(
            &[128, 128, 128, 37, 10, 30, 70, 255],
            2,
            1,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| e.to_string())
}
