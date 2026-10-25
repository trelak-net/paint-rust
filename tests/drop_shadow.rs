use paint_rust::{
    document::Selection,
    effects::{self, Effect},
};
use std::sync::atomic::AtomicBool;
fn effect(radius: u32, offset: [i32; 2], alpha: u8) -> Effect {
    Effect::DropShadow {
        radius,
        offset,
        color: [10, 20, 30, alpha],
    }
}
#[test]
fn offset_shadow_is_behind_source_and_retains_color_opacity() {
    let mut pixels = vec![[99, 88, 77, 0]; 25];
    pixels[6] = [200, 100, 50, 255];
    let output = effects::render(
        &pixels,
        5,
        5,
        None,
        effect(0, [1, 1], 128),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(output[6], pixels[6]);
    assert_eq!(output[12], [10, 20, 30, 128]);
    for i in 0..25 {
        if i != 12 {
            assert_eq!(output[i], pixels[i]);
        }
    }
    assert_eq!(
        effects::render(
            &pixels,
            5,
            5,
            None,
            effect(4, [1, 1], 0),
            &AtomicBool::new(false)
        )
        .unwrap(),
        pixels
    );
}
#[test]
fn soft_shadow_respects_selection_and_hidden_rgb_does_not_leak() {
    let mut pixels = vec![[255, 0, 0, 0]; 25];
    pixels[12] = [200, 100, 50, 255];
    let output = effects::render(
        &pixels,
        5,
        5,
        None,
        effect(2, [0, 0], 255),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(output[12], pixels[12]);
    assert!(
        output
            .iter()
            .enumerate()
            .any(|(i, p)| i != 12 && p[3] > 0 && p[3] < 255)
    );
    for (i, pixel) in output.iter().enumerate() {
        if i != 12 && pixel[3] > 0 {
            assert_eq!(pixel[..3], [10, 20, 30]);
        }
    }
    let selection = Selection {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
        mask: None,
        ellipse: None,
    };
    let selected = effects::render(
        &pixels,
        5,
        5,
        Some(selection),
        effect(2, [0, 0], 255),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(&selected[1..], &pixels[1..]);
}
#[test]
fn off_canvas_shadows_bounds_and_cancel_are_safe() {
    let pixel = [[20, 40, 60, 37]];
    assert_eq!(
        effects::render(
            &pixel,
            1,
            1,
            None,
            effect(0, [-200, 200], 255),
            &AtomicBool::new(false)
        )
        .unwrap(),
        pixel
    );
    for invalid in [effect(65, [0, 0], 255), effect(0, [201, 0], 255)] {
        assert!(!invalid.valid_settings());
        assert!(effects::render(&pixel, 1, 1, None, invalid, &AtomicBool::new(false)).is_none());
    }
    assert!(
        effects::render(
            &pixel,
            1,
            1,
            None,
            effect(64, [0, 0], 255),
            &AtomicBool::new(true)
        )
        .is_none()
    );
}
