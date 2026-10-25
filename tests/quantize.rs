use paint_rust::{
    document::Selection,
    effects::{self, Effect},
};
use std::{collections::HashSet, sync::atomic::AtomicBool};
fn run(pixels: &[[u8; 4]], colors: u16) -> Vec<[u8; 4]> {
    effects::render(
        pixels,
        pixels.len() as u32,
        1,
        None,
        Effect::Quantize(colors),
        &AtomicBool::new(false),
    )
    .unwrap()
}
#[test]
fn small_palettes_are_byte_identical_even_inside_one_histogram_cell() {
    let image = [
        [100, 100, 100, 255],
        [101, 102, 103, 83],
        [100, 100, 100, 1],
        [250, 0, 0, 0],
    ];
    assert_eq!(run(&image, 2), image);
    assert_eq!(run(&[[20, 30, 40, 0]; 5], 2), [[20, 30, 40, 0]; 5]);
}
#[test]
fn quantization_is_deterministic_bounded_and_preserves_alpha_and_hidden_rgb() {
    let image: Vec<_> = (0..1024u32)
        .map(|i| {
            [
                (i * 73 % 256) as u8,
                (i * 31 % 255) as u8,
                (i * 137 % 253) as u8,
                (i % 256) as u8,
            ]
        })
        .collect();
    for count in [2, 16, 64, 256] {
        let output = run(&image, count);
        assert_eq!(output, run(&image, count));
        let palette: HashSet<_> = output
            .iter()
            .filter(|p| p[3] > 0)
            .map(|p| [p[0], p[1], p[2]])
            .collect();
        assert!(palette.len() <= count as usize);
        assert!(palette.len() > 1);
        for (before, after) in image.iter().zip(&output) {
            assert_eq!(before[3], after[3]);
            if before[3] == 0 {
                assert_eq!(before, after);
            }
        }
    }
}
#[test]
fn transparent_colors_do_not_influence_quantization() {
    let image = [
        [0, 0, 0, 255],
        [255, 255, 255, 255],
        [128, 128, 128, 255],
        [255, 0, 0, 0],
    ];
    let first = run(&image, 2);
    let mut changed = image;
    changed[3] = [0, 255, 0, 0];
    let second = run(&changed, 2);
    assert_eq!(first[..3], second[..3]);
    assert!(first[..3].iter().all(|p| p[0] == p[1] && p[1] == p[2]));
}
#[test]
fn selection_cancellation_and_parameter_bounds_are_respected() {
    let pixels = [[0, 0, 0, 255], [255, 255, 255, 255], [128, 128, 128, 255]];
    let full = run(&pixels, 2);
    let selected = effects::render(
        &pixels,
        3,
        1,
        Some(Selection {
            x: 1,
            y: 0,
            width: 1,
            height: 1,
            mask: None,
            ellipse: None,
        }),
        Effect::Quantize(2),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(selected, [pixels[0], full[1], pixels[2]]);
    assert!(
        effects::render(
            &pixels,
            3,
            1,
            None,
            Effect::Quantize(16),
            &AtomicBool::new(true)
        )
        .is_none()
    );
    for colors in [0, 1, 257, u16::MAX] {
        assert!(!Effect::Quantize(colors).valid_settings());
        assert!(
            effects::render(
                &pixels,
                3,
                1,
                None,
                Effect::Quantize(colors),
                &AtomicBool::new(false)
            )
            .is_none()
        );
    }
}
