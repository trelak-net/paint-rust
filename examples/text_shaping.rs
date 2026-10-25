//! Render an installed font with several scripts for manual typography review.
use paint_rust::text::{self, TextLayout, TextStyle};
use std::sync::atomic::AtomicBool;
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    let font = text::load_font_cancellable(std::path::Path::new(&args[1]), &AtomicBool::new(false))
        .unwrap();
    let mut pixels = vec![[255; 4]; 1000 * 280];
    text::render(
        &mut pixels,
        1000,
        280,
        None,
        TextStyle {
            font: &font,
            text: "Office: ffi fi fl — Żółć e\u{301}\nمرحبا بالعالم 123 — Hello\nשלום עולם 123 — Hello",
            origin: (20.0, 20.0),
            size: 48.0,
            color: [20, 25, 40, 255],
            layout: TextLayout {
                underline: true,
                line_spacing: 1.5,
                ..Default::default()
            },
        },
    );
    let bytes: Vec<_> = pixels.into_iter().flatten().collect();
    image::save_buffer(&args[2], &bytes, 1000, 280, image::ColorType::Rgba8).unwrap();
}
