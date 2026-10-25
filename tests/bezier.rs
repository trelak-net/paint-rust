use paint_rust::{
    bezier::{self, Parameters},
    document::{Selection, Shape, ShapeStyle},
    shapes,
};
use std::sync::atomic::AtomicBool;
fn style() -> ShapeStyle {
    ShapeStyle {
        paint_mode: Some(Default::default()),
        antialias: true,
        outline: None,
        color: [20, 80, 180, 128],
        width: 4.0,
        filled: false,
    }
}
#[test]
fn straight_cubic_matches_line_and_control_points_do_not_accumulate_opacity() {
    let source = vec![[0; 4]; 64 * 64];
    let cancel = AtomicBool::new(false);
    let p = Parameters {
        points: [(8.0, 32.0), (24.0, 32.0), (40.0, 32.0), (56.0, 32.0)],
        style: style(),
    };
    let curve = bezier::render(&source, 64, 64, None, p, &cancel).unwrap();
    let line = shapes::render(
        &source,
        64,
        64,
        None,
        shapes::Parameters {
            shape: Shape::Line,
            from: p.points[0],
            to: p.points[3],
            style: p.style,
        },
        &cancel,
    )
    .unwrap();
    assert_eq!(curve, line);
    let p = Parameters {
        points: [(8.0, 32.0), (8.0, 0.0), (56.0, 64.0), (56.0, 32.0)],
        ..p
    };
    let curve = bezier::render(&source, 64, 64, None, p, &cancel).unwrap();
    assert!(curve.iter().any(|p| p[3] == 128));
    assert!(curve.iter().all(|p| p[3] <= 128));
    let reverse = bezier::render(
        &source,
        64,
        64,
        None,
        Parameters {
            points: [p.points[3], p.points[2], p.points[1], p.points[0]],
            ..p
        },
        &cancel,
    )
    .unwrap();
    assert_eq!(curve, reverse);
}
#[test]
fn collinear_reversal_is_not_collapsed_and_selection_is_respected() {
    let source = vec![[0; 4]; 64 * 64];
    let cancel = AtomicBool::new(false);
    let p = Parameters {
        points: [(20.0, 32.0), (100.0, 32.0), (-60.0, 32.0), (20.0, 32.0)],
        style: style(),
    };
    let curve = bezier::render(&source, 64, 64, None, p, &cancel).unwrap();
    assert!(curve[32 * 64 + 40][3] > 0);
    let selection = Selection::elliptical(24, 24, 16, 16);
    let clipped = bezier::render(&source, 64, 64, Some(&selection), p, &cancel).unwrap();
    for (index, pixel) in clipped.iter().enumerate() {
        if !(24..40).contains(&(index % 64)) || !(24..40).contains(&(index / 64)) {
            assert_eq!(*pixel, [0; 4]);
        }
    }
    assert!(bezier::render(&source, 64, 64, None, p, &AtomicBool::new(true)).is_none());
    assert!(
        bezier::render(
            &source,
            64,
            64,
            None,
            Parameters {
                points: [(f32::NAN, 0.0); 4],
                ..p
            },
            &cancel
        )
        .is_none()
    );
    assert!(
        bezier::render(
            &source,
            64,
            64,
            None,
            Parameters {
                points: [(1e20, 0.0); 4],
                ..p
            },
            &cancel
        )
        .is_none()
    );
}
