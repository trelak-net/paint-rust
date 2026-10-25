use paint_rust::{
    document::{Document, Selection},
    effects::{self, Effect},
};
use std::sync::atomic::AtomicBool;

#[test]
fn ellipse_limits_fill_delete_and_clipboard_without_rectangular_corners() {
    let mut doc = Document::new(5, 5, [255; 4]).unwrap();
    let selection = Selection::elliptical(0, 0, 5, 5);
    doc.set_selection(Some(selection.clone()));
    assert!(!doc.dirty());
    doc.begin("Fill");
    doc.fill(2, 2, [10, 20, 30, 255]);
    doc.commit();
    for index in [0, 4, 20, 24] {
        assert_eq!(doc.pixels()[index], [255; 4]);
    }
    assert_eq!(doc.pixels()[12], [10, 20, 30, 255]);
    let (width, height, copied) = doc.copy_pixels(false);
    assert_eq!((width, height), (5, 5));
    assert_eq!(copied[0], [0; 4]);
    assert_eq!(copied[12], [10, 20, 30, 255]);
    doc.clear_pixels();
    assert_eq!(doc.pixels()[0], [255; 4]);
    assert_eq!(doc.pixels()[12], [0; 4]);
    doc.undo();
    assert_eq!(doc.pixels()[12], [10, 20, 30, 255]);
    assert_eq!(doc.selection(), Some(selection));
}

#[test]
fn clipped_ellipse_keeps_original_geometry_when_selection_moves() {
    let mut doc = Document::new(5, 5, [255; 4]).unwrap();
    doc.set_selection(Some(Selection::elliptical(0, 0, 5, 5)));
    doc.begin("Move selection");
    doc.preview_move(-2, 0, false, true);
    assert!(doc.commit());
    let selection = doc.selection().unwrap();
    assert_eq!((selection.x, selection.width), (0, 3));
    assert_eq!(selection.ellipse.unwrap().x, -2);
    assert_eq!(selection.ellipse.unwrap().width, 5);
    doc.invert();
    assert_eq!(doc.pixels()[0], [0, 0, 0, 255]);
    assert_eq!(doc.pixels()[2], [255; 4]);
    doc.undo();
    assert!(!doc.dirty());
    assert!(doc.undo());
    assert_eq!(doc.selection(), Some(Selection::elliptical(0, 0, 5, 5)));
    assert!(!doc.dirty());
}

#[test]
fn moving_elliptical_pixels_leaves_unselected_corners_and_cancels_exactly() {
    let mut doc = Document::new(8, 5, [12, 24, 36, 255]).unwrap();
    let selection = Selection::elliptical(0, 0, 5, 5);
    doc.set_selection(Some(selection.clone()));
    let before = doc.pixels().to_vec();
    doc.begin("Move");
    doc.preview_move(3, 0, false, false);
    assert_eq!(doc.pixels()[0], before[0]);
    assert_eq!(doc.pixels()[2], [0; 4]);
    assert_eq!(doc.pixels()[5], before[2]);
    doc.cancel();
    assert_eq!(doc.pixels(), before);
    assert_eq!(doc.selection(), Some(selection));
    assert!(!doc.dirty());
}

#[test]
fn ellipse_effect_mask_and_canvas_clipping_preserve_shape() {
    let mut doc = Document::new(3, 5, [200, 100, 50, 123]).unwrap();
    doc.set_selection(Some(Selection::elliptical(0, 0, 5, 5)));
    let selection = doc.selection().unwrap();
    assert_eq!(selection.width, 3);
    assert_eq!(selection.ellipse.unwrap().width, 5);
    let output = effects::render(
        doc.pixels(),
        3,
        5,
        Some(selection),
        Effect::Grayscale,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(output[0], [200, 100, 50, 123]);
    assert_eq!(output[2][0], output[2][1]);
    assert_eq!(output[2][1], output[2][2]);
    assert_eq!(output[2][3], 123);
}
