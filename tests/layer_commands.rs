use paint_rust::{
    document::{BlendMode, Document, Selection},
    transform::Orientation,
};
use std::sync::atomic::AtomicBool;
#[test]
fn layer_orientation_is_exact_independent_of_selection_and_undoable() {
    let pixels: Vec<_> = (1..=6)
        .map(|i| [i * 20, i * 10, i, if i == 2 { 0 } else { i * 30 }])
        .collect();
    for (operation, order) in [
        (Orientation::FlipHorizontal, [2, 1, 0, 5, 4, 3]),
        (Orientation::FlipVertical, [3, 4, 5, 0, 1, 2]),
        (Orientation::HalfTurn, [5, 4, 3, 2, 1, 0]),
    ] {
        let mut doc = Document::from_pixels(3, 2, pixels.clone()).unwrap();
        doc.add_layer(true);
        doc.set_layer_properties("Detail".into(), 83);
        doc.set_layer_style("Detail".into(), 83, BlendMode::Multiply);
        let selection = Selection::elliptical(0, 0, 1, 1);
        doc.set_selection(Some(selection.clone()));
        doc.mark_saved();
        let before = doc.layers().to_vec();
        let history = doc.history().count();
        assert!(
            doc.reorient_active_cancellable(operation, &AtomicBool::new(false))
                .unwrap()
        );
        assert_eq!(doc.pixels(), order.map(|i| pixels[i]));
        assert!(doc.layers()[0] == before[0]);
        assert_eq!(doc.layers()[1].name, "Detail");
        assert_eq!(doc.layers()[1].opacity, 83);
        assert_eq!(doc.layers()[1].blend_mode, BlendMode::Multiply);
        assert_eq!(doc.selection(), Some(selection));
        assert_eq!((doc.width, doc.height), (3, 2));
        assert_eq!(doc.history().count(), history + 1);
        assert!(doc.undo());
        assert!(doc.layers() == before);
        assert!(!doc.dirty());
        assert!(doc.redo());
        assert_eq!(doc.pixels(), order.map(|i| pixels[i]));
    }
}
#[test]
fn selected_fill_replaces_rgba_only_in_mask_and_retains_other_layers() {
    let mut doc = Document::new(5, 5, [200, 100, 50, 0]).unwrap();
    doc.add_layer(true);
    let lower = doc.layers()[0].clone();
    let selection = Selection::elliptical(0, 0, 5, 5);
    doc.set_selection(Some(selection.clone()));
    doc.mark_saved();
    assert!(
        doc.fill_selection_cancellable([10, 20, 30, 37], &AtomicBool::new(false))
            .unwrap()
    );
    assert_eq!(doc.pixels()[12], [10, 20, 30, 37]);
    for i in [0, 4, 20, 24] {
        assert_eq!(doc.pixels()[i], [200, 100, 50, 0]);
    }
    assert!(doc.layers()[0] == lower);
    assert_eq!(doc.selection(), Some(selection));
    assert!(doc.undo());
    assert_eq!(doc.pixels(), &[[200, 100, 50, 0]; 25]);
    assert!(!doc.dirty());
    doc.set_selection(None);
    assert!(
        doc.fill_selection_cancellable([1, 2, 3, 4], &AtomicBool::new(false))
            .unwrap()
    );
    assert_eq!(doc.pixels(), &[[1, 2, 3, 4]; 25]);
}
#[test]
fn cancellation_and_noops_do_not_change_history_or_pixels() {
    let mut doc = Document::new(3, 2, [30, 50, 70, 83]).unwrap();
    assert!(
        doc.reorient_active_cancellable(Orientation::HalfTurn, &AtomicBool::new(true))
            .is_err()
    );
    assert!(
        doc.fill_selection_cancellable([1, 2, 3, 4], &AtomicBool::new(true))
            .is_err()
    );
    assert!(
        !doc.reorient_active_cancellable(Orientation::HalfTurn, &AtomicBool::new(false))
            .unwrap()
    );
    assert!(
        !doc.fill_selection_cancellable([30, 50, 70, 83], &AtomicBool::new(false))
            .unwrap()
    );
    assert!(!doc.can_undo());
    assert!(!doc.dirty());
    assert_eq!(doc.pixels(), &[[30, 50, 70, 83]; 6]);
}
