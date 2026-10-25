use paint_rust::{
    document::Selection,
    effects::{self, Effect},
};
use std::sync::atomic::AtomicBool;
fn exposure(stops: f32, offset: f32, gamma: f32) -> Effect {
    Effect::Exposure {
        stops,
        offset,
        gamma,
    }
}
#[test]
fn neutral_exposure_preserves_all_bytes() {
    let pixels: Vec<_> = (0..=255).map(|v| [v, 255 - v, v / 2, v]).collect();
    assert_eq!(
        effects::render(
            &pixels,
            256,
            1,
            None,
            exposure(0.0, 0.0, 1.0),
            &AtomicBool::new(false)
        )
        .unwrap(),
        pixels
    );
}
#[test]
fn exposure_uses_linear_light_and_preserves_alpha_hidden_rgb_and_selection() {
    let pixels = [[128, 64, 0, 37], [128, 64, 255, 0], [128, 64, 255, 255]];
    let selection = Selection {
        x: 0,
        y: 0,
        width: 2,
        height: 1,
        mask: None,
        ellipse: None,
    };
    let output = effects::render(
        &pixels,
        3,
        1,
        Some(selection),
        exposure(1.0, 0.0, 1.0),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(output, [[176, 90, 0, 37], pixels[1], pixels[2]]);
    assert_eq!(
        effects::render(
            &[[255, 0, 128, 255]],
            1,
            1,
            None,
            exposure(-1.0, 0.0, 1.0),
            &AtomicBool::new(false)
        )
        .unwrap(),
        [[188, 0, 92, 255]]
    );
}
#[test]
fn invalid_parameters_and_cancel_are_rejected() {
    for effect in [
        exposure(f32::NAN, 0.0, 1.0),
        exposure(11.0, 0.0, 1.0),
        exposure(0.0, 0.0, 0.0),
        exposure(0.0, f32::INFINITY, 1.0),
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
            exposure(1.0, 0.0, 1.0),
            &AtomicBool::new(true)
        )
        .is_none()
    );
}
#[test]
fn offset_and_gamma_are_bounded_and_monotonic() {
    let pixels: Vec<_> = (0..=255).map(|v| [v, v, v, 255]).collect();
    for effect in [
        exposure(0.0, 0.1, 1.0),
        exposure(0.0, -0.1, 1.0),
        exposure(0.0, 0.0, 2.0),
        exposure(10.0, 1.0, 0.1),
    ] {
        let output =
            effects::render(&pixels, 256, 1, None, effect, &AtomicBool::new(false)).unwrap();
        assert!(output.windows(2).all(|p| p[0][0] <= p[1][0]));
        assert!(
            output
                .iter()
                .all(|p| p[0] == p[1] && p[1] == p[2] && p[3] == 255)
        );
    }
}
