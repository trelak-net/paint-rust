use paint_rust::document::{Document, GradientStyle, Selection};

#[test]
fn linear_gradient_hits_endpoints_and_replaces_older_preview() {
    let mut doc = Document::new(4, 1, [0; 4]).unwrap();
    doc.begin("Gradient");
    doc.preview_gradient(
        (0.5, 0.5),
        (3.5, 0.5),
        GradientStyle {
            start: [255, 0, 0, 255],
            end: [0, 0, 255, 255],
            shape: paint_rust::gradient::GradientShape::Linear,
            mode: paint_rust::gradient::GradientMode::Blend,
        },
    );
    assert_eq!(
        doc.pixels(),
        &[
            [255, 0, 0, 255],
            [170, 0, 85, 255],
            [85, 0, 170, 255],
            [0, 0, 255, 255]
        ]
    );
    doc.preview_gradient(
        (3.5, 0.5),
        (0.5, 0.5),
        GradientStyle {
            start: [255, 0, 0, 255],
            end: [0, 0, 255, 255],
            shape: paint_rust::gradient::GradientShape::Linear,
            mode: paint_rust::gradient::GradientMode::Blend,
        },
    );
    assert_eq!(doc.pixels()[0], [0, 0, 255, 255]);
    assert!(doc.commit());
    assert_eq!(doc.history().count(), 1);
    doc.undo();
    assert!(!doc.dirty());
    assert!(doc.pixels().iter().all(|p| *p == [0; 4]));
}

#[test]
fn transparent_gradient_does_not_bleed_hidden_rgb_and_respects_selection() {
    let mut doc = Document::new(3, 1, [0; 4]).unwrap();
    doc.set_selection(Some(Selection {
        mask: None,
        ellipse: None,
        x: 1,
        y: 0,
        width: 2,
        height: 1,
    }));
    doc.begin("Gradient");
    doc.preview_gradient(
        (0.5, 0.5),
        (2.5, 0.5),
        GradientStyle {
            start: [255, 0, 0, 255],
            end: [0, 0, 255, 0],
            shape: paint_rust::gradient::GradientShape::Linear,
            mode: paint_rust::gradient::GradientMode::Blend,
        },
    );
    assert_eq!(doc.pixels(), &[[0; 4], [255, 0, 0, 128], [0; 4]]);
    doc.cancel();
    assert!(!doc.dirty());
}

#[test]
fn radial_gradient_uses_distance_from_center() {
    let mut doc = Document::new(3, 3, [0; 4]).unwrap();
    doc.begin("Gradient");
    doc.preview_gradient(
        (1.5, 1.5),
        (2.5, 1.5),
        GradientStyle {
            start: [255; 4],
            end: [0, 0, 0, 255],
            shape: paint_rust::gradient::GradientShape::Radial,
            mode: paint_rust::gradient::GradientMode::Blend,
        },
    );
    assert_eq!(doc.pixels()[4], [255; 4]);
    assert_eq!(doc.pixels()[0], [0, 0, 0, 255]);
    assert_eq!(doc.pixels()[3], doc.pixels()[5]);
    assert_eq!(doc.pixels()[1], doc.pixels()[7]);
}

#[test]
fn gradient_renderer_rejects_invalid_inputs_and_cancellation() {
    use paint_rust::gradient::{Gradient, render};
    use std::sync::atomic::AtomicBool;
    let source = [[17; 4]; 4];
    let parameters = Gradient {
        from: (0.0, 0.0),
        to: (2.0, 2.0),
        style: GradientStyle {
            start: [255; 4],
            end: [0; 4],
            shape: paint_rust::gradient::GradientShape::Radial,
            mode: paint_rust::gradient::GradientMode::Blend,
        },
    };
    assert!(render(&source, 2, 2, None, parameters, &AtomicBool::new(true)).is_none());
    assert!(render(&source, 3, 2, None, parameters, &AtomicBool::new(false)).is_none());
    assert!(
        render(
            &source,
            2,
            2,
            None,
            Gradient {
                from: (f32::NAN, 0.0),
                ..parameters
            },
            &AtomicBool::new(false)
        )
        .is_none()
    );
    assert_eq!(source, [[17; 4]; 4]);
}

#[test]
fn gradient_shapes_follow_the_drag_axis_and_conical_angle() {
    use paint_rust::gradient::{Gradient, GradientMode, GradientShape as S, render};
    use std::sync::atomic::AtomicBool;
    for (shape, diagonal) in [
        (S::Linear, 128),
        (S::Reflected, 128),
        (S::Radial, 180),
        (S::Diamond, 255),
        (S::Conical, 32),
    ] {
        let output = render(
            &[[0; 4]; 16],
            4,
            4,
            None,
            Gradient {
                from: (1.5, 1.5),
                to: (3.5, 1.5),
                style: GradientStyle {
                    start: [0, 0, 0, 255],
                    end: [255; 4],
                    shape,
                    mode: GradientMode::Replace,
                },
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            output[2 * 4 + 2],
            [diagonal, diagonal, diagonal, 255],
            "{shape:?}"
        );
        if shape == S::Reflected {
            assert_eq!(output[4], output[6]);
        }
        if shape == S::Conical {
            for (index, value) in [(6, 0), (9, 64), (4, 128), (1, 191)] {
                assert_eq!(output[index][0], value);
            }
        }
    }
}

#[test]
fn transparency_modulates_existing_alpha_and_replace_can_remove_pixels() {
    use paint_rust::gradient::{Gradient, GradientMode as M, GradientShape, render};
    use std::sync::atomic::AtomicBool;
    let source = [[80, 100, 120, 128]; 3];
    let parameters = Gradient {
        from: (0.5, 0.5),
        to: (2.5, 0.5),
        style: GradientStyle {
            start: [255, 0, 0, 255],
            end: [0, 0, 255, 0],
            shape: GradientShape::Linear,
            mode: M::Transparency,
        },
    };
    let output = render(&source, 3, 1, None, parameters, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        output,
        [[80, 100, 120, 128], [80, 100, 120, 64], [80, 100, 120, 0]]
    );
    let replaced = render(
        &source,
        3,
        1,
        None,
        Gradient {
            style: GradientStyle {
                mode: M::Replace,
                ..parameters.style
            },
            ..parameters
        },
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(replaced, [[255, 0, 0, 255], [255, 0, 0, 128], [0; 4]]);
    let blended = render(
        &source,
        3,
        1,
        None,
        Gradient {
            style: GradientStyle {
                mode: M::Blend,
                ..parameters.style
            },
            ..parameters
        },
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(blended[2], source[2]);
    assert_eq!(source, [[80, 100, 120, 128]; 3]);
}

#[test]
fn every_shape_handles_zero_length_and_extreme_finite_coordinates() {
    use paint_rust::gradient::{Gradient, GradientMode, GradientShape, render};
    use std::sync::atomic::AtomicBool;
    for shape in GradientShape::ALL {
        let parameters = Gradient {
            from: (0.5, 0.5),
            to: (0.5, 0.5),
            style: GradientStyle {
                start: [23, 45, 67, 255],
                end: [255; 4],
                shape,
                mode: GradientMode::Replace,
            },
        };
        assert_eq!(
            render(&[[0; 4]], 1, 1, None, parameters, &AtomicBool::new(false)).unwrap(),
            [[23, 45, 67, 255]]
        );
        assert!(
            render(
                &[[0; 4]],
                1,
                1,
                None,
                Gradient {
                    from: (-f32::MAX, 0.0),
                    to: (f32::MAX, 0.0),
                    ..parameters
                },
                &AtomicBool::new(false)
            )
            .is_some()
        );
    }
}
