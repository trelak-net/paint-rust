use paint_rust::{
    curves::{Curve, Curves},
    effects::{self, Effect},
};
use std::sync::atomic::AtomicBool;

#[test]
fn identity_inversion_and_control_points_are_exact() {
    let mut curve = Curve::default();
    assert_eq!(curve.table(), std::array::from_fn(|i| i as u8));
    curve.move_point(0, 0, 255);
    curve.move_point(1, 255, 0);
    assert_eq!(curve.table(), std::array::from_fn(|i| 255 - i as u8));
    for (x, y) in [(32, 240), (64, 40), (128, 200), (220, 10)] {
        curve.insert(x, y);
    }
    let table = curve.table();
    for &(x, y) in curve.points() {
        assert_eq!(table[x as usize], y);
    }
    for pair in curve.points().windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        assert!(
            table[x0 as usize..=x1 as usize]
                .iter()
                .all(|y| (y0.min(y1)..=y0.max(y1)).contains(y))
        );
    }
}

#[test]
fn editing_preserves_sorted_unique_points_and_bounded_storage() {
    let mut curve = Curve::default();
    assert!(!curve.remove(0));
    assert!(!curve.remove(1));
    for x in 1..=14 {
        assert!(curve.insert(x, x).is_some());
    }
    assert!(curve.insert(15, 15).is_none());
    assert_eq!(curve.insert(5, 99), Some(5));
    curve.move_point(5, 255, 80);
    assert!(curve.points().windows(2).all(|p| p[0].0 < p[1].0));
    assert!(curve.remove(5));
    assert!(curve.insert(128, 200).is_some());
    assert_eq!(curve.points().len(), 16);
    assert_eq!(curve.table()[128], 200);
}

#[test]
fn channels_compose_preserve_alpha_and_respect_masks_and_cancellation() {
    let mut curves = Curves::default();
    curves.channels[0].move_point(0, 0, 255);
    curves.channels[0].move_point(1, 255, 0);
    curves.channels[1].move_point(0, 0, 42);
    curves.channels[1].move_point(1, 255, 42);
    let source = [[10, 20, 30, 0], [100, 150, 200, 100]];
    let output = effects::render(
        &source,
        2,
        1,
        None,
        Effect::Curves(curves),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(output, [[42, 235, 225, 0], [42, 105, 55, 100]]);
    assert!(
        effects::render(
            &source,
            2,
            1,
            None,
            Effect::Curves(curves),
            &AtomicBool::new(true)
        )
        .is_none()
    );
    let selection = paint_rust::document::Selection {
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
        Effect::Curves(curves),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(output, [source[0], [42, 105, 55, 100]]);
}
