use paint_rust::effects::{self, Effect};
use std::sync::atomic::AtomicBool;
fn render(source: &[[u8; 4]], radius: u32, threshold: u8) -> Vec<[u8; 4]> {
    effects::render(
        source,
        source.len() as u32,
        1,
        None,
        Effect::SurfaceBlur { radius, threshold },
        &AtomicBool::new(false),
    )
    .unwrap()
}
#[test]
fn low_threshold_preserves_edges_but_smooths_similar_colors() {
    let pixels = [[50, 50, 50, 255], [70, 70, 70, 255], [240, 240, 240, 255]];
    let output = render(&pixels, 1, 30);
    assert!(output[0][0] > 50 && output[0][0] < 70);
    assert!(output[1][0] < 70 && output[1][0] > 50);
    assert_eq!(output[2], pixels[2]);
    assert_eq!(render(&pixels, 1, 1), pixels);
}
#[test]
fn opacity_and_hidden_colors_are_preserved() {
    let pixels = [[255, 0, 0, 0], [0, 0, 255, 128], [0, 0, 255, 255]];
    assert_eq!(render(&pixels, 10, 255), pixels);
    assert_eq!(render(&pixels, 0, 255), pixels);
    assert_eq!(render(&[[1, 2, 3, 4]], 10, 255), [[1, 2, 3, 4]]);
}
#[test]
fn bounds_and_cancellation_are_enforced() {
    for effect in [
        Effect::SurfaceBlur {
            radius: 11,
            threshold: 20,
        },
        Effect::SurfaceBlur {
            radius: 1,
            threshold: 0,
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
            Effect::SurfaceBlur {
                radius: 10,
                threshold: 255
            },
            &AtomicBool::new(true)
        )
        .is_none()
    );
}
