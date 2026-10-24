use paint_rust::document::{Document, Shape, ShapeStyle};

fn style(filled: bool) -> ShapeStyle {
    ShapeStyle {
        color: [25, 70, 160, 255],
        width: 2.0,
        filled,
    }
}

#[test]
fn preview_replaces_previous_shape_and_commits_one_history_entry() {
    let mut doc = Document::new(32, 32, [255; 4]).unwrap();
    doc.begin("Rectangle");
    doc.preview_shape(Shape::Rectangle, (2.0, 2.0), (28.0, 28.0), style(true));
    assert_ne!(doc.pixels()[25 * 32 + 25], [255; 4]);
    doc.preview_shape(Shape::Rectangle, (2.0, 2.0), (8.0, 8.0), style(true));
    assert_eq!(doc.pixels()[25 * 32 + 25], [255; 4]);
    assert!(doc.commit());
    assert_eq!(doc.history().count(), 1);
    doc.undo();
    assert!(doc.pixels().iter().all(|p| *p == [255; 4]));
    doc.redo();
    assert_ne!(doc.pixels()[4 * 32 + 4], [255; 4]);
}

#[test]
fn ellipse_outline_has_hollow_center_and_reverse_drag_matches() {
    let mut a = Document::new(24, 24, [0; 4]).unwrap();
    a.begin("Ellipse");
    a.preview_shape(Shape::Ellipse, (2.0, 4.0), (22.0, 20.0), style(false));
    assert_eq!(a.pixels()[12 * 24 + 12], [0; 4]);
    assert_ne!(a.pixels()[4 * 24 + 12], [0; 4]);
    let expected = a.pixels().to_vec();
    a.preview_shape(Shape::Ellipse, (22.0, 20.0), (2.0, 4.0), style(false));
    assert_eq!(a.pixels(), expected);
    a.cancel();
    assert!(!a.dirty());
    assert!(a.pixels().iter().all(|p| *p == [0; 4]));
}

#[test]
fn clipped_shapes_and_lines_modify_only_active_layer() {
    let mut doc = Document::new(8, 8, [255; 4]).unwrap();
    doc.add_layer(false);
    doc.begin("Line");
    doc.preview_shape(Shape::Line, (-100.0, 3.5), (100.0, 3.5), style(false));
    doc.commit();
    assert!(doc.layers()[0].pixels().iter().all(|p| *p == [255; 4]));
    for x in 0..8 {
        assert_eq!(doc.pixels()[3 * 8 + x], [25, 70, 160, 255]);
    }
    doc.begin("Off canvas");
    doc.preview_shape(Shape::Ellipse, (-20.0, -20.0), (-10.0, -10.0), style(true));
    assert!(!doc.commit());
}

#[test]
fn shape_kernels_reject_cancelled_or_invalid_requests() {
    use paint_rust::shapes::{Parameters, render};
    use std::sync::atomic::AtomicBool;
    let source = [[17; 4]; 64];
    for shape in [Shape::Line, Shape::Rectangle, Shape::Ellipse] {
        let parameters = Parameters {
            shape,
            from: (1.0, 1.0),
            to: (7.0, 7.0),
            style: style(true),
        };
        assert!(render(&source, 8, 8, None, parameters, &AtomicBool::new(true)).is_none());
        assert!(render(&source, 9, 8, None, parameters, &AtomicBool::new(false)).is_none());
        assert!(
            render(
                &source,
                8,
                8,
                None,
                Parameters {
                    to: (f32::INFINITY, 3.0),
                    ..parameters
                },
                &AtomicBool::new(false)
            )
            .is_none()
        );
    }
    assert_eq!(source, [[17; 4]; 64]);
}
