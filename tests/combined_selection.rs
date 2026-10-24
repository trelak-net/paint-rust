use paint_rust::{
    document::{Document, Selection},
    selection::{CombineMode, combine},
};

fn rectangle(x: u32, y: u32, width: u32, height: u32) -> Selection {
    Selection {
        x,
        y,
        width,
        height,
        mask: None,
        ellipse: None,
    }
}

#[test]
fn add_subtract_and_intersect_produce_pixel_masks_without_dirtying_selection() {
    let a = rectangle(0, 0, 3, 2);
    let b = rectangle(2, 1, 3, 2);
    for (mode, count) in [
        (CombineMode::Add, 11),
        (CombineMode::Subtract, 5),
        (CombineMode::Intersect, 1),
    ] {
        let mut doc = Document::new(5, 3, [255; 4]).unwrap();
        doc.set_selection(Some(combine(5, 3, Some(&a), &b, mode).unwrap()));
        assert!(!doc.dirty());
        doc.clear_pixels();
        assert_eq!(doc.pixels().iter().filter(|p| **p == [0; 4]).count(), count);
        doc.undo();
        assert!(!doc.dirty());
    }
}

#[test]
fn empty_intersection_stays_empty_and_never_means_select_all() {
    let a = rectangle(0, 0, 1, 1);
    let b = rectangle(3, 3, 1, 1);
    let empty = combine(4, 4, Some(&a), &b, CombineMode::Intersect).unwrap();
    let mut doc = Document::new(4, 4, [255; 4]).unwrap();
    doc.set_selection(Some(empty));
    assert_eq!(doc.selection().unwrap().width, 0);
    doc.invert();
    doc.clear_pixels();
    doc.begin("Fill");
    doc.fill(0, 0, [0; 4]);
    doc.commit();
    doc.begin("Move");
    doc.preview_move(1, 1, false, false);
    doc.commit();
    assert!(!doc.dirty());
    assert!(!doc.can_undo());
    assert!(!doc.crop_to_selection());
    assert_eq!(doc.copy_pixels(false), (0, 0, vec![]));
    assert!(doc.selection().is_some());
}

#[test]
fn ellipse_difference_clipboard_and_movement_keep_holes_and_undo() {
    let full = rectangle(0, 0, 5, 5);
    let ellipse = Selection::elliptical(0, 0, 5, 5);
    let mask = combine(5, 5, Some(&full), &ellipse, CombineMode::Subtract).unwrap();
    let mut doc = Document::new(5, 5, [20, 40, 60, 255]).unwrap();
    doc.set_selection(Some(mask.clone()));
    let (_, _, copied) = doc.copy_pixels(false);
    assert_eq!(copied[0], [20, 40, 60, 255]);
    assert_eq!(copied[12], [0; 4]);
    doc.begin("Move");
    doc.preview_move(1, 0, false, false);
    doc.commit();
    assert_eq!(doc.pixels()[0], [0; 4]);
    assert_eq!(doc.pixels()[1], [20, 40, 60, 255]);
    assert_eq!(doc.pixels()[12], [20, 40, 60, 255]);
    doc.undo();
    assert_eq!(doc.selection(), Some(mask));
    assert!(!doc.dirty());
    assert!(doc.history_bytes() >= 25 * 4 + 25);
}

#[test]
fn missing_previous_selection_has_predictable_combination_semantics() {
    let candidate = rectangle(1, 1, 1, 1);
    for (mode, count) in [
        (CombineMode::Add, 1),
        (CombineMode::Subtract, 8),
        (CombineMode::Intersect, 1),
    ] {
        let mut doc = Document::new(3, 3, [255; 4]).unwrap();
        doc.set_selection(Some(combine(3, 3, None, &candidate, mode).unwrap()));
        doc.clear_pixels();
        assert_eq!(doc.pixels().iter().filter(|p| **p == [0; 4]).count(), count);
    }
    assert!(combine(0, 0, None, &candidate, CombineMode::Add).is_err());
}
