use paint_rust::{
    document::Selection,
    effects::{self, Effect},
};
use std::sync::atomic::AtomicBool;
fn render(source: &[[u8; 4]], width: u32, effect: Effect) -> Vec<[u8; 4]> {
    effects::render(
        source,
        width,
        source.len() as u32 / width,
        None,
        effect,
        &AtomicBool::new(false),
    )
    .unwrap()
}
#[test]
fn neutral_adjustments_and_zero_blurs_are_identity() {
    let source = [[12, 56, 190, 0], [20, 30, 40, 128], [255, 255, 255, 255]];
    for effect in [
        Effect::GaussianBlur(0),
        Effect::MotionBlur {
            distance: 0,
            angle: 40.0,
            centered: false,
        },
        Effect::TemperatureTint {
            temperature: 0,
            tint: 0,
        },
        Effect::HighlightsShadows {
            highlights: 0,
            shadows: 0,
        },
    ] {
        assert_eq!(render(&source, 3, effect), source);
    }
    assert_eq!(
        render(
            &render(&source, 3, Effect::InvertAlpha),
            3,
            Effect::InvertAlpha
        ),
        source
    );
}
#[test]
fn blurs_ignore_hidden_rgb_and_preserve_constant_images() {
    for effect in [
        Effect::GaussianBlur(3),
        Effect::MotionBlur {
            distance: 4,
            angle: 0.0,
            centered: true,
        },
        Effect::MotionBlur {
            distance: 4,
            angle: 90.0,
            centered: false,
        },
    ] {
        let constant = vec![[50, 100, 200, 120]; 25];
        assert_eq!(render(&constant, 5, effect), constant);
        let mut source = vec![[255, 0, 0, 0]; 25];
        source[12] = [0, 0, 255, 255];
        let output = render(&source, 5, effect);
        assert!(output.iter().any(|p| p[3] > 0 && p[3] < 255));
        for pixel in output.iter().filter(|p| p[3] > 0) {
            assert_eq!(&pixel[..3], &[0, 0, 255]);
        }
        assert!(effects::render(&source, 5, 5, None, effect, &AtomicBool::new(true)).is_none());
    }
}
#[test]
fn gaussian_is_symmetric_and_motion_direction_is_observable() {
    let mut source = vec![[0; 4]; 49];
    source[24] = [200, 100, 50, 255];
    let gaussian = render(&source, 7, Effect::GaussianBlur(2));
    for y in 0..7 {
        for x in 0..7 {
            assert_eq!(gaussian[y * 7 + x], gaussian[x * 7 + y]);
            assert_eq!(gaussian[y * 7 + x], gaussian[(6 - y) * 7 + 6 - x]);
        }
    }
    let horizontal = render(
        &source,
        7,
        Effect::MotionBlur {
            distance: 4,
            angle: 0.0,
            centered: true,
        },
    );
    let vertical = render(
        &source,
        7,
        Effect::MotionBlur {
            distance: 4,
            angle: 90.0,
            centered: true,
        },
    );
    assert!(horizontal[23][3] > 0 && horizontal[17][3] == 0);
    assert!(vertical[17][3] > 0 && vertical[23][3] == 0);
}
#[test]
fn adjustments_preserve_alpha_and_mask_with_expected_direction() {
    let source = [[60, 60, 60, 12], [200, 200, 200, 140]];
    let warm = render(
        &source,
        2,
        Effect::TemperatureTint {
            temperature: 50,
            tint: 0,
        },
    );
    assert!(warm[0][0] > 60 && warm[0][2] < 60);
    assert_eq!(warm[0][1], 60);
    let lifted = render(
        &source,
        2,
        Effect::HighlightsShadows {
            highlights: 0,
            shadows: 50,
        },
    );
    assert!(lifted[0][0] - 60 > lifted[1][0] - 200);
    for effect in [
        Effect::InvertAlpha,
        Effect::GaussianBlur(3),
        Effect::TemperatureTint {
            temperature: 100,
            tint: -100,
        },
        Effect::HighlightsShadows {
            highlights: -50,
            shadows: 50,
        },
    ] {
        let selection = Selection {
            x: 1,
            y: 0,
            width: 1,
            height: 1,
            mask: None,
            ellipse: None,
        };
        let output = effects::render(
            &source,
            2,
            1,
            Some(selection),
            effect,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(output[0], source[0]);
        if matches!(
            effect,
            Effect::TemperatureTint { .. } | Effect::HighlightsShadows { .. }
        ) {
            assert_eq!(output[1][3], source[1][3]);
        }
    }
    assert!(
        effects::render(
            &source,
            2,
            1,
            None,
            Effect::MotionBlur {
                distance: 3,
                angle: f32::NAN,
                centered: true
            },
            &AtomicBool::new(false)
        )
        .is_none()
    );
}
