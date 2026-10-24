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
            radial: false,
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
            radial: false,
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
            radial: false,
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
            radial: true,
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
            radial: true,
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
