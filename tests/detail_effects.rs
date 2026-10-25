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
        Effect::ReduceNoise {
            radius: 1,
            strength: 1.0,
        },
        Effect::Relief {
            angle: 0.0,
            strength: 1.0,
        },
        Effect::Outline {
            radius: 1,
            intensity: 1.0,
        },
    ]
}
#[test]
fn median_denoising_removes_isolated_noise_and_blends_strength() {
    let pixels = [[30, 50, 70, 255], [255, 255, 255, 255], [30, 50, 70, 255]];
    assert_eq!(run(&pixels, effects()[0]), [[30, 50, 70, 255]; 3]);
    assert_eq!(
        run(
            &pixels,
            Effect::ReduceNoise {
                radius: 1,
                strength: 0.5
            }
        )[1],
        [143, 153, 163, 255]
    );
    for effect in [
        Effect::ReduceNoise {
            radius: 0,
            strength: 1.0,
        },
        Effect::ReduceNoise {
            radius: 10,
            strength: 0.0,
        },
        Effect::Relief {
            angle: 45.0,
            strength: 0.0,
        },
    ] {
        assert_eq!(run(&pixels, effect), pixels);
    }
}
#[test]
fn outlines_detect_contrast_and_relief_retains_color_with_directional_shading() {
    let flat = [[50, 80, 120, 83]; 5];
    assert_eq!(run(&flat, effects()[1]), flat);
    assert_eq!(run(&flat, effects()[2]), [[255, 255, 255, 83]; 5]);
    let pixels = [[0, 0, 0, 255], [80, 100, 120, 255], [255, 255, 255, 255]];
    assert_eq!(run(&pixels, effects()[2])[1], [0, 0, 0, 255]);
    let right = run(&pixels, effects()[1]);
    let left = run(
        &pixels,
        Effect::Relief {
            angle: 180.0,
            strength: 1.0,
        },
    );
    assert!(right[1][0] > pixels[1][0]);
    assert!(left[1][0] < pixels[1][0]);
    assert_eq!(right[1][1] - right[1][0], 20);
}
#[test]
fn detail_effects_preserve_alpha_hidden_rgb_selection_and_serialized_parameters() {
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
        Effect::ReduceNoise {
            radius: 11,
            strength: 0.5,
        },
        Effect::Relief {
            angle: f32::NAN,
            strength: 1.0,
        },
        Effect::Outline {
            radius: 0,
            intensity: 1.0,
        },
    ] {
        assert!(!effect.valid_settings());
        assert!(effects::render(&pixels, 3, 1, None, effect, &AtomicBool::new(false)).is_none());
    }
}
