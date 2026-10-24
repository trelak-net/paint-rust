use paint_rust::document::{Document, Selection};

#[test]
fn moving_overlapping_pixels_uses_frozen_source_and_undo_restores_selection() {
    let mut doc = Document::from_pixels(
        5,
        1,
        vec![
            [10, 0, 0, 255],
            [20, 0, 0, 255],
            [30, 0, 0, 255],
            [40, 0, 0, 255],
            [50, 0, 0, 255],
        ],
    )
    .unwrap();
    let before = doc.pixels().to_vec();
    let area = Selection {
        ellipse: None,
        x: 0,
        y: 0,
        width: 3,
        height: 1,
    };
    doc.set_selection(Some(area));
    doc.begin("Move");
    doc.preview_move(1, 0, false, false);
    assert_eq!(
        doc.pixels().iter().map(|p| p[0]).collect::<Vec<_>>(),
        vec![0, 10, 20, 30, 50]
    );
    doc.preview_move(2, 0, false, false);
    assert_eq!(
        doc.pixels().iter().map(|p| p[0]).collect::<Vec<_>>(),
        vec![0, 0, 10, 20, 30]
    );
    assert!(doc.commit());
    assert_eq!(doc.history().count(), 1);
    doc.undo();
    assert_eq!(doc.pixels(), before);
    assert_eq!(doc.selection(), Some(area));
}

#[test]
fn copy_and_selection_only_moves_do_not_clear_source_pixels() {
    let mut doc = Document::new(4, 4, [255; 4]).unwrap();
    doc.set_selection(Some(Selection {
        ellipse: None,
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    }));
    doc.begin("Copy");
    doc.preview_move(2, 2, true, false);
    assert_eq!(doc.pixels()[0], [255; 4]);
    doc.cancel();
    doc.begin("Move selection");
    doc.preview_move(2, 2, false, true);
    assert!(!doc.commit());
    assert!(!doc.dirty());
    assert_eq!(
        doc.selection(),
        Some(Selection {
            ellipse: None,
            x: 2,
            y: 2,
            width: 1,
            height: 1
        })
    );
}

#[test]
fn clipped_moves_cancel_cleanly_and_zero_offset_is_exact() {
    let mut doc = Document::new(4, 4, [12, 34, 56, 0]).unwrap();
    doc.begin("Move");
    doc.preview_move(0, 0, false, false);
    assert!(!doc.commit());
    doc.begin("Move");
    doc.preview_move(i32::MAX, i32::MIN, false, false);
    assert_eq!(doc.selection(), None);
    assert!(doc.pixels().iter().all(|p| *p == [0; 4]));
    doc.cancel();
    assert!(!doc.dirty());
    assert!(doc.pixels().iter().all(|p| *p == [12, 34, 56, 0]));
}
