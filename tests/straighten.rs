use paint_rust::{
    document::Selection,
    effects::{self, Effect},
};
use std::sync::atomic::AtomicBool;
fn run(source: &[[u8; 4]], w: u32, h: u32, angle: f32, auto_crop: bool) -> Vec<[u8; 4]> {
    effects::render(
        source,
        w,
        h,
        None,
        Effect::Straighten { angle, auto_crop },
        &AtomicBool::new(false),
    )
    .unwrap()
}
#[test]
fn zero_angle_preserves_every_byte_and_auto_crop_keeps_opaque_corners() {
    let source: Vec<_> = (0..35).map(|i| [i * 7, 255 - i, 50, i * 6]).collect();
    for crop in [true, false] {
        assert_eq!(run(&source, 7, 5, 0.0, crop), source);
    }
    for (w, h) in [(7, 5), (5, 7), (1, 7), (7, 1), (1, 1)] {
        for angle in [-45.0, -12.5, 12.5, 45.0] {
            let image = vec![[40, 90, 170, 255]; (w * h) as usize];
            assert_eq!(run(&image, w, h, angle, true), image);
        }
    }
    let image = vec![[40, 90, 170, 255]; 35];
    assert!(run(&image, 7, 5, 30.0, false).iter().any(|p| p[3] < 255));
}
#[test]
fn rotation_keeps_center_and_turns_details_in_the_expected_direction() {
    let mut source = vec![[0; 4]; 49];
    source[24] = [0, 0, 255, 255];
    source[25] = [0, 255, 0, 255];
    let output = run(&source, 7, 7, 45.0, false);
    assert_eq!(output[24], source[24]);
    assert!(output[32][1] > 0 && output[32][3] > 0);
    assert_eq!(output[18][3], 0);
}
#[test]
fn straighten_selection_cancellation_and_parameter_limits() {
    let source: Vec<_> = (0..25).map(|i| [i * 10, i, 100, 255]).collect();
    let effect = Effect::Straighten {
        angle: 30.0,
        auto_crop: true,
    };
    let full = run(&source, 5, 5, 30.0, true);
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
        assert_eq!(selected[i], if i == 6 { full[i] } else { source[i] });
    }
    assert!(effects::render(&source, 5, 5, None, effect, &AtomicBool::new(true)).is_none());
    assert_eq!(
        serde_json::from_slice::<Effect>(&serde_json::to_vec(&effect).unwrap()).unwrap(),
        effect
    );
    for angle in [f32::NAN, 46.0, -46.0] {
        let effect = Effect::Straighten {
            angle,
            auto_crop: true,
        };
        assert!(!effect.valid_settings());
        assert!(effects::render(&source, 5, 5, None, effect, &AtomicBool::new(false)).is_none());
    }
}
