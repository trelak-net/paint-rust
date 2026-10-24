use paint_rust::document::{Document, Selection, Shape, ShapeStyle};

fn area() -> Selection {
    Selection {
        mask: None,
        ellipse: None,
        x: 2,
        y: 2,
        width: 3,
        height: 3,
    }
}

#[test]
fn brush_fill_shapes_and_invert_respect_the_selection() {
    for operation in 0..4 {
        let mut doc = Document::new(8, 8, [255; 4]).unwrap();
        doc.set_selection(Some(area()));
        match operation {
            0 => {
                doc.begin("Brush");
                doc.stroke((0.0, 4.0), (8.0, 4.0), 16.0, [0, 0, 0, 255], false);
                doc.commit();
            }
            1 => {
                doc.begin("Fill");
                doc.fill(3, 3, [0, 0, 0, 255]);
                doc.commit();
            }
            2 => {
                doc.begin("Shape");
                doc.preview_shape(
                    Shape::Rectangle,
                    (0.0, 0.0),
                    (8.0, 8.0),
                    ShapeStyle {
                        color: [0, 0, 0, 255],
                        width: 1.0,
                        filled: true,
                    },
                );
                doc.commit();
            }
            _ => doc.invert(),
        }
        for y in 0..8 {
            for x in 0..8 {
                let expected = if (2..5).contains(&x) && (2..5).contains(&y) {
                    [0, 0, 0, 255]
                } else {
                    [255; 4]
                };
                assert_eq!(
                    doc.pixels()[y * 8 + x],
                    expected,
                    "operation {operation}, ({x},{y})"
                );
            }
        }
    }
}

#[test]
fn selection_changes_are_clean_and_fill_outside_is_a_noop() {
    let mut doc = Document::new(8, 8, [255; 4]).unwrap();
    doc.set_selection(Some(area()));
    assert!(!doc.dirty());
    assert!(!doc.can_undo());
    doc.begin("Fill");
    doc.fill(0, 0, [0; 4]);
    assert!(!doc.commit());
    doc.clear_pixels();
    assert_eq!(doc.pixels()[3 * 8 + 3], [0; 4]);
    doc.undo();
    assert!(!doc.dirty());
    assert_eq!(doc.selection(), Some(area()));
}

#[test]
fn crop_preserves_all_layers_and_undo_restores_dimensions_and_selection() {
    let pixels = (0..64).map(|i| [i, i + 1, i + 2, 255]).collect();
    let mut doc = Document::from_pixels(8, 8, pixels).unwrap();
    doc.add_layer(true);
    doc.set_layer_properties("Upper".into(), 128);
    doc.set_visibility(1, false);
    doc.set_selection(Some(area()));
    let before = doc.layers()[0].pixels().to_vec();
    assert!(doc.crop_to_selection());
    assert_eq!((doc.width, doc.height), (3, 3));
    assert_eq!(doc.selection(), None);
    assert_eq!(doc.layers()[0].pixels()[0], before[2 * 8 + 2]);
    assert_eq!(doc.layers()[1].name, "Upper");
    assert!(!doc.layers()[1].visible);
    doc.undo();
    assert_eq!((doc.width, doc.height), (8, 8));
    assert_eq!(doc.selection(), Some(area()));
    assert_eq!(doc.layers()[0].pixels(), before);
    doc.redo();
    assert_eq!((doc.width, doc.height), (3, 3));
    assert_eq!(doc.selection(), None);
}

#[test]
fn selection_bounds_are_clipped_without_integer_overflow() {
    let mut doc = Document::new(8, 8, [255; 4]).unwrap();
    doc.set_selection(Some(Selection {
        mask: None,
        ellipse: None,
        x: 7,
        y: 7,
        width: u32::MAX,
        height: u32::MAX,
    }));
    assert_eq!(
        doc.selection(),
        Some(Selection {
            mask: None,
            ellipse: None,
            x: 7,
            y: 7,
            width: 1,
            height: 1
        })
    );
    doc.set_selection(Some(Selection {
        mask: None,
        ellipse: None,
        x: u32::MAX,
        y: 0,
        width: 10,
        height: 10,
    }));
    assert_eq!(doc.selection(), None);
}

#[test]
fn cancelled_fill_can_be_rolled_back_without_history_or_dirty_state() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let mut document = Document::new(32, 32, [255; 4]).unwrap();
    let cancel = AtomicBool::new(true);
    document.begin("Fill");
    assert!(!document.fill_cancellable(0, 0, [0; 4], &cancel));
    document.cancel();
    assert!(!document.dirty());
    assert_eq!(document.history().count(), 0);
    assert!(document.pixels().iter().all(|p| *p == [255; 4]));
    cancel.store(false, Ordering::Relaxed);
    document.begin("Fill");
    assert!(document.fill_cancellable(0, 0, [0; 4], &cancel));
    document.commit();
    assert!(document.pixels().iter().all(|p| *p == [0; 4]));
}

#[test]
fn cancelled_document_operations_leave_the_original_unchanged() {
    use std::sync::atomic::AtomicBool;
    let mut document = Document::new(8, 8, [10, 20, 30, 255]).unwrap();
    document.add_layer(false);
    let before = document.snapshot();
    let history = document.history().count();
    let cancel = AtomicBool::new(true);
    assert!(!document.invert_cancellable(&cancel));
    assert_eq!(document.flatten_cancellable(&cancel), None);
    assert!(document.layers() == before.layers());
    assert_eq!(document.history().count(), history);
    assert!(!document.editing());
}

#[test]
fn cancelled_crop_preserves_geometry_selection_and_history() {
    let mut doc = Document::new(8, 8, [17; 4]).unwrap();
    doc.set_selection(Some(area()));
    let cancel = std::sync::atomic::AtomicBool::new(true);
    assert_eq!(
        doc.crop_to_selection_cancellable(&cancel).unwrap_err(),
        paint_rust::files::CANCELLED
    );
    assert_eq!((doc.width, doc.height), (8, 8));
    assert_eq!(doc.selection(), Some(area()));
    assert_eq!(doc.pixels(), &[[17; 4]; 64]);
    assert!(!doc.can_undo());
    assert!(!doc.dirty());
}

#[test]
fn cancelled_pixel_deletion_preserves_selection_and_pixels() {
    let mut doc = Document::new(8, 8, [17; 4]).unwrap();
    doc.set_selection(Some(area()));
    assert!(!doc.clear_pixels_cancellable(&std::sync::atomic::AtomicBool::new(true)));
    assert_eq!(doc.selection(), Some(area()));
    assert_eq!(doc.pixels(), &[[17; 4]; 64]);
    assert!(!doc.can_undo());
    assert!(!doc.dirty());
}
