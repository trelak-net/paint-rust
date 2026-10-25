use paint_rust::{
    document::Selection,
    effects::{self, Effect},
};
use std::sync::atomic::AtomicBool;
fn effect(radius: u32, percentile: u8, smoothness: u8, seed: u32) -> Effect {
    Effect::SketchBlur {
        radius,
        percentile,
        smoothness,
        seed,
    }
}
fn run(source: &[[u8; 4]], w: u32, h: u32, effect: Effect) -> Vec<[u8; 4]> {
    effects::render(source, w, h, None, effect, &AtomicBool::new(false)).unwrap()
}
#[test]
fn sketch_blur_preserves_constants_alpha_hidden_rgb_and_zero_radius() {
    for percentile in [0, 50, 100] {
        for smoothness in 1..=5 {
            assert_eq!(
                run(
                    &[[40, 90, 170, 83]; 25],
                    5,
                    5,
                    effect(10, percentile, smoothness, 1)
                ),
                [[40, 90, 170, 83]; 25]
            );
        }
    }
    let pixels = [[255, 0, 0, 0], [0, 0, 255, 37], [255, 0, 0, 0]];
    assert_eq!(run(&pixels, 3, 1, effect(10, 50, 5, 1)), pixels);
    assert_eq!(run(&pixels, 3, 1, effect(0, 50, 5, 1)), pixels);
}
#[test]
fn sketch_sampling_is_repeatable_seeded_and_percentiles_control_brightness() {
    let pixels: Vec<_> = (0..225u32)
        .map(|i| {
            [
                (i * 73 % 256) as u8,
                (i * 31 % 255) as u8,
                (i * 137 % 253) as u8,
                255,
            ]
        })
        .collect();
    let settings = effect(4, 50, 2, 1);
    let first = run(&pixels, 15, 15, settings);
    assert_eq!(first, run(&pixels, 15, 15, settings));
    assert_ne!(first, pixels);
    assert_ne!(first, run(&pixels, 15, 15, effect(4, 50, 2, 2)));
    let low = run(&pixels, 15, 15, effect(4, 0, 2, 1));
    let high = run(&pixels, 15, 15, effect(4, 100, 2, 1));
    for i in 0..pixels.len() {
        for c in 0..3 {
            assert!(low[i][c] <= first[i][c] && first[i][c] <= high[i][c]);
        }
    }
}
#[test]
fn sketch_selection_cancel_roundtrip_and_limits() {
    let pixels: Vec<_> = (0..25).map(|i| [i * 10, i, 50, 255]).collect();
    let settings = effect(4, 50, 2, 1);
    let full = run(&pixels, 5, 5, settings);
    let selected = effects::render(
        &pixels,
        5,
        5,
        Some(Selection {
            x: 1,
            y: 1,
            width: 1,
            height: 1,
            mask: None,
            ellipse: None,
        }),
        settings,
        &AtomicBool::new(false),
    )
    .unwrap();
    for i in 0..25 {
        assert_eq!(selected[i], if i == 6 { full[i] } else { pixels[i] });
    }
    assert!(effects::render(&pixels, 5, 5, None, settings, &AtomicBool::new(true)).is_none());
    assert_eq!(
        serde_json::from_slice::<Effect>(&serde_json::to_vec(&settings).unwrap()).unwrap(),
        settings
    );
    for settings in [
        effect(101, 50, 2, 1),
        effect(4, 101, 2, 1),
        effect(4, 50, 0, 1),
    ] {
        assert!(!settings.valid_settings());
        assert!(effects::render(&pixels, 5, 5, None, settings, &AtomicBool::new(false)).is_none());
    }
}
