use paint_rust::{
    document::Selection,
    effects::{self, Effect},
};
use std::sync::atomic::AtomicBool;
fn run(source: &[[u8; 4]], effect: Effect) -> Vec<[u8; 4]> {
    effects::render(source, 5, 5, None, effect, &AtomicBool::new(false)).unwrap()
}
fn warps() -> [Effect; 3] {
    [
        Effect::Dents {
            scale: 3.0,
            amount: 3.0,
            seed: 1,
        },
        Effect::PolarInversion {
            amount: 1.0,
            center: [0.5, 0.5],
        },
        Effect::TileReflection {
            size: 4.0,
            amount: 1.0,
            angle: 0.0,
        },
    ]
}
#[test]
fn neutral_warps_preserve_exact_bytes_including_hidden_rgb() {
    let source: Vec<_> = (0..25).map(|i| [i * 10, 255 - i, 50, i * 9]).collect();
    for effect in [
        Effect::Dents {
            scale: 3.0,
            amount: 0.0,
            seed: 1,
        },
        Effect::PolarInversion {
            amount: 0.0,
            center: [0.2, 0.8],
        },
        Effect::TileReflection {
            size: 4.0,
            amount: 0.0,
            angle: 45.0,
        },
    ] {
        assert_eq!(run(&source, effect), source);
    }
}
#[test]
fn warps_preserve_constant_pixels_and_do_not_mix_hidden_colors() {
    for effect in warps() {
        assert_eq!(
            run(&[[40, 90, 170, 83]; 25], effect),
            [[40, 90, 170, 83]; 25]
        );
        let mut source = vec![[255, 0, 0, 0]; 25];
        source[12] = [0, 0, 255, 255];
        let output = run(&source, effect);
        assert!(
            output
                .iter()
                .filter(|p| p[3] > 0)
                .all(|p| p[0] == 0 && p[1] == 0 && p[2] == 255)
        );
    }
}
#[test]
fn inversion_and_tile_displacement_have_expected_geometry() {
    let source: Vec<_> = (0..25).map(|i| [i * 10, i, 50, 255]).collect();
    let inverted = run(&source, warps()[1]);
    assert_eq!(inverted[12], source[12]);
    assert_eq!(inverted[13], source[14]);
    assert_eq!(inverted[11], source[10]);
    let tiled = run(&source, warps()[2]);
    assert_eq!(tiled[12], source[12]);
    assert_eq!(tiled[13], source[14]);
    assert_eq!(tiled[11], source[10]);
    let first = run(&source, warps()[0]);
    assert_eq!(first, run(&source, warps()[0]));
    assert_ne!(
        first,
        run(
            &source,
            Effect::Dents {
                scale: 3.0,
                amount: 3.0,
                seed: 2
            }
        )
    );
}
#[test]
fn warp_selection_cancellation_validation_and_settings_roundtrip() {
    let source: Vec<_> = (0..25).map(|i| [i * 10, i, 50, 255]).collect();
    for effect in warps() {
        let selected = effects::render(
            &source,
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
            effect,
            &AtomicBool::new(false),
        )
        .unwrap();
        for i in 0..25 {
            if i != 6 {
                assert_eq!(selected[i], source[i]);
            }
        }
        assert!(effects::render(&source, 5, 5, None, effect, &AtomicBool::new(true)).is_none());
        assert_eq!(
            serde_json::from_slice::<Effect>(&serde_json::to_vec(&effect).unwrap()).unwrap(),
            effect
        );
        assert_eq!(
            effects::render(
                &[[17, 29, 35, 41]],
                1,
                1,
                None,
                effect,
                &AtomicBool::new(false)
            )
            .unwrap(),
            [[17, 29, 35, 41]]
        );
    }
    for effect in [
        Effect::Dents {
            scale: 0.0,
            amount: 3.0,
            seed: 1,
        },
        Effect::PolarInversion {
            amount: 1.0,
            center: [f32::NAN, 0.5],
        },
        Effect::TileReflection {
            size: 4.0,
            amount: 1.0,
            angle: f32::INFINITY,
        },
    ] {
        assert!(!effect.valid_settings());
        assert!(effects::render(&source, 5, 5, None, effect, &AtomicBool::new(false)).is_none());
    }
}
