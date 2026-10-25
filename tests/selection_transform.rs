use paint_rust::{
    document::{Document, Selection},
    selection::{self, MaskTransform},
};
use std::sync::atomic::AtomicBool;
fn rect(x: u32, y: u32, width: u32, height: u32) -> Selection {
    Selection {
        x,
        y,
        width,
        height,
        mask: None,
        ellipse: None,
    }
}
fn transform(source: &Selection, p: MaskTransform) -> Selection {
    selection::transform_cancellable(10, 10, source, p, &AtomicBool::new(false)).unwrap()
}
fn mask(selection: Selection) -> Vec<bool> {
    let mut doc = Document::new(10, 10, [0, 0, 0, 255]).unwrap();
    doc.set_selection(Some(selection));
    doc.invert();
    doc.pixels().iter().map(|p| p[0] == 255).collect()
}
#[test]
fn boundary_scale_rotation_and_translation_match_expected_geometry() {
    let source = rect(2, 2, 2, 2);
    assert_eq!(
        mask(transform(
            &source,
            MaskTransform {
                scale: [2.0; 2],
                ..Default::default()
            }
        )),
        mask(rect(1, 1, 4, 4))
    );
    assert_eq!(
        mask(transform(
            &source,
            MaskTransform {
                offset: [3.0, 1.0],
                ..Default::default()
            }
        )),
        mask(rect(5, 3, 2, 2))
    );
    assert_eq!(
        mask(transform(
            &rect(2, 2, 4, 2),
            MaskTransform {
                angle: 90.0,
                ..Default::default()
            }
        )),
        mask(rect(3, 1, 2, 4))
    );
}
#[test]
fn boundary_transform_preserves_ellipse_and_irregular_mask_details() {
    let ellipse = Selection::elliptical(2, 2, 4, 4);
    assert_eq!(transform(&ellipse, Default::default()), ellipse);
    let moved = transform(
        &ellipse,
        MaskTransform {
            offset: [2.0, 1.0],
            ..Default::default()
        },
    );
    assert_eq!(mask(moved), mask(Selection::elliptical(4, 3, 4, 4)));
    let polygon = selection::polygon_cancellable(
        10,
        10,
        &[(2.0, 2.0), (6.0, 2.0), (2.0, 6.0)],
        &AtomicBool::new(false),
    )
    .unwrap();
    let flipped = transform(
        &polygon,
        MaskTransform {
            flip: [true, false],
            ..Default::default()
        },
    );
    assert_ne!(mask(flipped.clone()), mask(polygon.clone()));
    let restored = transform(
        &flipped,
        MaskTransform {
            flip: [true, false],
            ..Default::default()
        },
    );
    assert_eq!(mask(restored), mask(polygon));
}
#[test]
fn transforms_clip_to_canvas_and_keep_empty_selection_explicit() {
    let source = rect(2, 2, 4, 4);
    assert_eq!(
        mask(transform(
            &source,
            MaskTransform {
                offset: [-4.0, 0.0],
                ..Default::default()
            }
        )),
        mask(rect(0, 2, 2, 4))
    );
    let empty = transform(
        &source,
        MaskTransform {
            offset: [100.0, 0.0],
            ..Default::default()
        },
    );
    assert_eq!((empty.width, empty.height), (0, 0));
    assert!(empty.mask.is_some());
    assert!(mask(empty).iter().all(|value| !*value));
}
#[test]
fn invalid_and_cancelled_transforms_leave_source_unchanged() {
    let source = Selection::elliptical(2, 2, 4, 4);
    let before = source.clone();
    assert!(
        selection::transform_cancellable(
            10,
            10,
            &source,
            Default::default(),
            &AtomicBool::new(true)
        )
        .is_err()
    );
    for p in [
        MaskTransform {
            scale: [0.0, 1.0],
            ..Default::default()
        },
        MaskTransform {
            angle: f32::NAN,
            ..Default::default()
        },
        MaskTransform {
            offset: [f32::INFINITY, 0.0],
            ..Default::default()
        },
    ] {
        assert!(
            selection::transform_cancellable(10, 10, &source, p, &AtomicBool::new(false)).is_err()
        );
    }
    assert_eq!(source, before);
}
