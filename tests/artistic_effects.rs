use paint_rust::{
    document::Selection,
    effects::{self, Effect},
};
use std::sync::atomic::AtomicBool;
fn run(pixels: &[[u8; 4]], effect: Effect) -> Vec<[u8; 4]> {
    effects::render(
        pixels,
        pixels.len() as u32,
        1,
        None,
        effect,
        &AtomicBool::new(false),
    )
    .unwrap()
}
fn effects() -> [Effect; 3] {
    [
        Effect::OilPainting {
            radius: 1,
            bins: 16,
        },
        Effect::PencilSketch {
            radius: 2,
            strength: 1.0,
        },
        Effect::InkSketch {
            threshold: 40,
            color: 1.0,
        },
    ]
}
#[test]
fn oil_uses_dominant_alpha_weighted_tonal_group_and_retains_destination_alpha() {
    let blue = [0, 0, 255, 255];
    let red = [255, 0, 0, 255];
    assert_eq!(run(&[blue, red, blue], effects()[0]), [blue; 3]);
    let translucent = [255, 0, 0, 1];
    let output = run(&[blue, translucent, blue], effects()[0]);
    assert_eq!(output[1], [0, 0, 255, 1]);
    assert_eq!(
        run(&[[10, 20, 30, 83]; 5], effects()[0]),
        [[10, 20, 30, 83]; 5]
    );
    let hidden = [255, 0, 0, 0];
    let output = run(&[blue, hidden, blue], effects()[0]);
    assert_eq!(output, [blue, hidden, blue]);
}
#[test]
fn pencil_is_monochrome_at_full_strength_and_leaves_flat_regions_white() {
    assert_eq!(
        run(&[[30, 50, 70, 83]; 5], effects()[1]),
        [[255, 255, 255, 83]; 5]
    );
    assert_eq!(run(&[[0, 0, 0, 255]; 5], effects()[1]), [[255; 4]; 5]);
    let pixels = [[200, 220, 240, 255], [0, 0, 0, 255], [200, 220, 240, 255]];
    let output = run(&pixels, effects()[1]);
    assert_eq!(output[1], [0, 0, 0, 255]);
    assert!(output.iter().all(|p| p[0] == p[1] && p[1] == p[2]));
    assert_eq!(
        run(
            &pixels,
            Effect::PencilSketch {
                radius: 2,
                strength: 0.0
            }
        ),
        pixels
    );
}
#[test]
fn ink_threshold_controls_edges_and_color_amount_controls_grayscale() {
    let pixels = [[0, 0, 0, 255], [80, 100, 120, 255], [255, 255, 255, 255]];
    assert_eq!(run(&pixels, effects()[2])[1], [0, 0, 0, 255]);
    assert_eq!(
        run(
            &pixels,
            Effect::InkSketch {
                threshold: 255,
                color: 1.0
            }
        ),
        pixels
    );
    let gray = run(
        &pixels,
        Effect::InkSketch {
            threshold: 255,
            color: 0.0,
        },
    );
    assert!(gray.iter().all(|p| p[0] == p[1] && p[1] == p[2]));
    assert_eq!(gray[1], [97, 97, 97, 255]);
}
#[test]
fn artistic_selection_hidden_pixels_cancel_and_settings_validation() {
    let pixels = [[30, 50, 70, 83], [255, 0, 0, 0], [100, 120, 140, 128]];
    for effect in effects() {
        let output = run(&pixels, effect);
        assert_eq!(output[1], pixels[1]);
        for i in 0..3 {
            assert_eq!(output[i][3], pixels[i][3]);
        }
        let selected = effects::render(
            &pixels,
            3,
            1,
            Some(Selection {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                mask: None,
                ellipse: None,
            }),
            effect,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(selected[1..], pixels[1..]);
        assert_eq!(selected[0], output[0]);
        assert_eq!(
            serde_json::from_slice::<Effect>(&serde_json::to_vec(&effect).unwrap()).unwrap(),
            effect
        );
        assert!(effects::render(&pixels, 3, 1, None, effect, &AtomicBool::new(true)).is_none());
    }
    for effect in [
        Effect::OilPainting { radius: 1, bins: 0 },
        Effect::PencilSketch {
            radius: 65,
            strength: 1.0,
        },
        Effect::InkSketch {
            threshold: 40,
            color: f32::NAN,
        },
    ] {
        assert!(!effect.valid_settings());
        assert!(effects::render(&pixels, 3, 1, None, effect, &AtomicBool::new(false)).is_none());
    }
}
