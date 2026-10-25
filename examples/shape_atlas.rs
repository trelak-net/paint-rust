//! Render the original shape catalogue for visual coverage checks.
use paint_rust::{
    document::{Shape, ShapeStyle},
    shapes::{self, Parameters},
};
use std::sync::atomic::AtomicBool;
fn main() {
    let path = std::env::args().nth(1).expect("output PNG path");
    let cell = 128usize;
    let width = cell * 6;
    let height = cell * Shape::ALL.len().div_ceil(6);
    let mut atlas = vec![255u8; width * height * 4];
    for (index, shape) in Shape::ALL.into_iter().enumerate() {
        let pixels = shapes::render(
            &vec![[255; 4]; cell * cell],
            cell as u32,
            cell as u32,
            None,
            Parameters {
                shape,
                from: (10.0, 10.0),
                to: (118.0, 118.0),
                style: ShapeStyle {
                    paint_mode: None,
                    antialias: true,
                    outline: Some([20, 55, 90, 255]),
                    color: [80, 170, 220, 255],
                    width: 3.0,
                    filled: true,
                },
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        for y in 0..cell {
            let target = ((index / 6 * cell + y) * width + index % 6 * cell) * 4;
            atlas[target..target + cell * 4]
                .copy_from_slice(pixels[y * cell..(y + 1) * cell].as_flattened());
        }
        println!(
            "Row {}, column {}: {}",
            index / 6 + 1,
            index % 6 + 1,
            shape.label()
        );
    }
    image::save_buffer(
        path,
        &atlas,
        width as u32,
        height as u32,
        image::ColorType::Rgba8,
    )
    .unwrap();
}
