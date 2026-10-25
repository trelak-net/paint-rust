use paint_rust::effects::{self, Effect};
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
#[test]
fn neutral_photo_effects_preserve_hidden_rgb_and_partial_alpha() {
    let pixels = [[20, 40, 80, 37], [250, 100, 30, 0], [180, 190, 200, 255]];
    for effect in [
        Effect::Glow {
            radius: 10,
            amount: 0.0,
        },
        Effect::SoftenPortrait {
            radius: 10,
            softness: 0.0,
            warmth: 0.0,
        },
    ] {
        assert_eq!(run(&pixels, effect), pixels);
    }
}
#[test]
fn glow_brightens_without_changing_alpha_or_tinting_from_hidden_pixels() {
    let pixels = [[0, 0, 200, 255], [255, 0, 0, 0], [0, 0, 80, 128]];
    let output = run(
        &pixels,
        Effect::Glow {
            radius: 2,
            amount: 1.0,
        },
    );
    assert_eq!(output[1], pixels[1]);
    for i in [0, 2] {
        assert_eq!(output[i][3], pixels[i][3]);
        assert_eq!(output[i][0], 0);
        assert_eq!(output[i][1], 0);
        assert!(output[i][2] > pixels[i][2]);
    }
}
#[test]
fn portrait_softens_details_and_warmth_changes_color_balance() {
    let pixels = [[0, 0, 0, 255], [200, 200, 200, 255], [0, 0, 0, 255]];
    let soft = run(
        &pixels,
        Effect::SoftenPortrait {
            radius: 2,
            softness: 1.0,
            warmth: 0.0,
        },
    );
    assert!(soft[0][0] > 0);
    assert!(soft[1][0] < 200);
    let warm = run(
        &[[100, 100, 100, 37]],
        Effect::SoftenPortrait {
            radius: 0,
            softness: 0.0,
            warmth: 1.0,
        },
    );
    assert_eq!(warm, [[124, 104, 76, 37]]);
}
#[test]
fn invalid_settings_and_cancel_are_rejected() {
    for effect in [
        Effect::Glow {
            radius: 65,
            amount: 1.0,
        },
        Effect::SoftenPortrait {
            radius: 1,
            softness: f32::NAN,
            warmth: 0.0,
        },
    ] {
        assert!(!effect.valid_settings());
        assert!(effects::render(&[[1; 4]], 1, 1, None, effect, &AtomicBool::new(false)).is_none());
    }
    assert!(
        effects::render(
            &[[1; 4]],
            1,
            1,
            None,
            Effect::Glow {
                radius: 64,
                amount: 1.0
            },
            &AtomicBool::new(true)
        )
        .is_none()
    );
}
