use paint_rust::{
    document::Selection,
    effects::{self, Effect, FractalSettings, NoiseSettings},
};
use std::sync::atomic::AtomicBool;
fn run(w: u32, h: u32, effect: Effect) -> Vec<[u8; 4]> {
    effects::render(
        &vec![[31, 47, 83, 19]; (w * h) as usize],
        w,
        h,
        None,
        effect,
        &AtomicBool::new(false),
    )
    .unwrap()
}
fn generators() -> [Effect; 4] {
    [
        Effect::Clouds(NoiseSettings::default()),
        Effect::Turbulence(NoiseSettings::default()),
        Effect::Mandelbrot(FractalSettings::default()),
        Effect::Julia(FractalSettings::default()),
    ]
}
#[test]
fn noise_is_deterministic_seeded_and_distinct_from_turbulence() {
    let settings = NoiseSettings {
        scale: 8.0,
        ..Default::default()
    };
    let first = run(32, 24, Effect::Clouds(settings));
    assert_eq!(first, run(32, 24, Effect::Clouds(settings)));
    assert_ne!(
        first,
        run(
            32,
            24,
            Effect::Clouds(NoiseSettings {
                seed: 2,
                ..settings
            })
        )
    );
    assert_ne!(first, run(32, 24, Effect::Turbulence(settings)));
    assert_ne!(
        first,
        run(
            32,
            24,
            Effect::Clouds(NoiseSettings {
                roughness: 0.0,
                ..settings
            })
        )
    );
    assert!(
        first
            .iter()
            .all(|p| p[0] == p[1] && p[1] == p[2] && p[3] == 255)
    );
    assert!(first.windows(2).any(|p| p[0] != p[1]));
}
#[test]
fn fractal_known_interior_and_exterior_points_and_conjugate_symmetry() {
    let settings = FractalSettings {
        center: [0.0, 0.0],
        ..Default::default()
    };
    assert_eq!(
        run(1, 1, Effect::Mandelbrot(settings)),
        [settings.colors[0]]
    );
    assert_eq!(
        run(
            1,
            1,
            Effect::Mandelbrot(FractalSettings {
                center: [3.0, 0.0],
                ..settings
            })
        ),
        [settings.colors[1]]
    );
    assert_eq!(
        run(
            1,
            1,
            Effect::Julia(FractalSettings {
                julia: [0.0, 0.0],
                ..settings
            })
        ),
        [settings.colors[0]]
    );
    assert_eq!(
        run(
            1,
            1,
            Effect::Julia(FractalSettings {
                julia: [0.0, 0.0],
                center: [3.0, 0.0],
                ..settings
            })
        ),
        [settings.colors[1]]
    );
    let image = run(31, 21, Effect::Mandelbrot(settings));
    for y in 0..21 {
        for x in 0..31 {
            assert_eq!(image[y * 31 + x], image[(20 - y) * 31 + x]);
        }
    }
    assert_ne!(
        image,
        run(
            31,
            21,
            Effect::Mandelbrot(FractalSettings {
                zoom: 2.0,
                ..settings
            })
        )
    );
}
#[test]
fn generator_palette_interpolation_is_alpha_aware() {
    for effect in [
        Effect::Clouds(NoiseSettings {
            scale: 5.0,
            colors: [[255, 0, 0, 0], [0, 0, 255, 255]],
            ..Default::default()
        }),
        Effect::Turbulence(NoiseSettings {
            scale: 5.0,
            colors: [[255, 0, 0, 0], [0, 0, 255, 255]],
            ..Default::default()
        }),
    ] {
        let output = run(20, 20, effect);
        assert!(
            output
                .iter()
                .filter(|p| p[3] > 0)
                .all(|p| p[0] == 0 && p[1] == 0 && p[2] == 255)
        );
        assert!(output.iter().any(|p| p[3] > 0 && p[3] < 255));
    }
}
#[test]
fn generators_replace_only_selection_and_roundtrip_parameters() {
    let source = vec![[30, 40, 50, 0]; 35];
    let selection = Selection::elliptical(1, 1, 5, 3);
    for effect in generators() {
        let full = run(7, 5, effect);
        let output = effects::render(
            &source,
            7,
            5,
            Some(selection.clone()),
            effect,
            &AtomicBool::new(false),
        )
        .unwrap();
        for y in 0..5 {
            for x in 0..7 {
                let i = y * 7 + x;
                assert_eq!(
                    output[i],
                    if matches!((x, y), (2..=4, 1 | 3) | (1..=5, 2)) {
                        full[i]
                    } else {
                        source[i]
                    }
                );
            }
        }
        assert!(output.iter().any(|p| p[3] > 0));
        let bytes = serde_json::to_vec(&effect).unwrap();
        assert_eq!(serde_json::from_slice::<Effect>(&bytes).unwrap(), effect);
        assert!(effects::render(&source, 7, 5, None, effect, &AtomicBool::new(true)).is_none());
    }
}
#[test]
fn generators_reject_unbounded_or_nonfinite_settings() {
    for effect in [
        Effect::Clouds(NoiseSettings {
            scale: f32::NAN,
            ..Default::default()
        }),
        Effect::Turbulence(NoiseSettings {
            roughness: 1.1,
            ..Default::default()
        }),
        Effect::Mandelbrot(FractalSettings {
            iterations: 513,
            ..Default::default()
        }),
        Effect::Julia(FractalSettings {
            center: [f32::INFINITY, 0.0],
            ..Default::default()
        }),
    ] {
        assert!(!effect.valid_settings());
        assert!(effects::render(&[[0; 4]], 1, 1, None, effect, &AtomicBool::new(false)).is_none());
    }
}
