//! Verify weight-axis rendering with a supplied local variable font.
use paint_rust::text::{self, TextLayout, TextStyle, variations};
use std::sync::atomic::AtomicBool;
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    let font = text::load_font_cancellable(std::path::Path::new(&args[1]), &AtomicBool::new(false))
        .unwrap();
    let axis = variations::axes(&font, 0)
        .into_iter()
        .find(|axis| axis.tag == *b"wght")
        .expect("Font needs a weight axis");
    let mut pixels = vec![[255; 4]; 800 * 200];
    let mut ink = Vec::new();
    for (index, value) in [axis.min_value, axis.max_value].into_iter().enumerate() {
        let mut settings = variations::Settings::default();
        assert!(settings.set(*b"wght", value));
        assert!(text::render_cancellable(
            &mut pixels,
            800,
            200,
            None,
            TextStyle {
                font: &font,
                text: "Variable weight: Office ffi",
                origin: (10.0, 10.0 + index as f32 * 90.0),
                size: 64.0,
                color: [0, 0, 0, 255],
                layout: TextLayout {
                    font_axes: settings,
                    ..Default::default()
                }
            },
            &AtomicBool::new(false)
        ));
        ink.push(
            pixels[index * 90 * 800..(index + 1) * 90 * 800]
                .iter()
                .map(|p| u64::from(255 - p[0]))
                .sum::<u64>(),
        );
    }
    assert!(
        ink[1] * 10 > ink[0] * 11,
        "Weight must change raster coverage: {ink:?}"
    );
    println!(
        "Weight range {}..{}: ink coverage {ink:?}",
        axis.min_value, axis.max_value
    );
    let bytes: Vec<_> = pixels.into_iter().flatten().collect();
    image::save_buffer(&args[2], &bytes, 800, 200, image::ColorType::Rgba8).unwrap();
}
