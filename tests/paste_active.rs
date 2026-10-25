use paint_rust::{
    document::{Document, Layer, Selection},
    files,
};
use std::sync::atomic::AtomicBool;
#[test]
fn active_paste_replaces_rgba_beyond_old_mask_and_restores_selection_on_undo() {
    let mut doc = Document::new(5, 4, [20, 30, 40, 255]).unwrap();
    doc.add_layer(true);
    let bottom = doc.layers()[0].clone();
    let old = Selection::elliptical(2, 1, 1, 1);
    doc.set_selection(Some(old.clone()));
    doc.mark_saved();
    let source = [
        [255, 0, 0, 0],
        [0, 0, 255, 37],
        [10, 20, 30, 128],
        [40, 50, 60, 255],
    ];
    assert!(
        doc.paste_active_cancellable(2, 2, &source, &AtomicBool::new(false))
            .unwrap()
    );
    assert_eq!(doc.layers().len(), 2);
    assert!(doc.layers()[0] == bottom);
    for (index, pixel) in [7, 8, 12, 13].into_iter().zip(source) {
        assert_eq!(doc.pixels()[index], pixel);
    }
    let pasted = doc.selection().unwrap();
    assert_eq!(
        (pasted.x, pasted.y, pasted.width, pasted.height),
        (2, 1, 2, 2)
    );
    assert!(pasted.ellipse.is_none());
    assert_eq!(doc.pixels()[0], [20, 30, 40, 255]);
    assert!(doc.undo());
    assert_eq!(doc.selection(), Some(old));
    assert_eq!(doc.pixels(), &[[20, 30, 40, 255]; 20]);
    assert!(!doc.dirty());
    assert!(doc.redo());
    assert_eq!(doc.pixels()[7], [255, 0, 0, 0]);
}
#[test]
fn paste_centers_without_selection_clips_when_requested_and_works_at_layer_limit() {
    let layers = (0..32)
        .map(|i| Layer::new(format!("Layer {i}"), vec![[20, 30, 40, 255]; 12]))
        .collect();
    let mut doc = Document::from_layers(4, 3, layers, 31).unwrap();
    assert!(!doc.can_add_layer());
    doc.paste_active_cancellable(2, 1, &[[1, 2, 3, 4]; 2], &AtomicBool::new(false))
        .unwrap();
    assert_eq!(&doc.pixels()[5..7], &[[1, 2, 3, 4]; 2]);
    assert_eq!(doc.layers().len(), 32);
    doc.set_selection(None);
    let image: Vec<_> = (0..30).map(|i| [i, 0, 0, 255]).collect();
    doc.paste_active_cancellable(6, 5, &image, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        doc.pixels(),
        &[0, 1, 2, 3, 6, 7, 8, 9, 12, 13, 14, 15].map(|i| [i, 0, 0, 255])
    );
    assert_eq!((doc.width, doc.height), (4, 3));
}
#[test]
fn cancelled_and_invalid_pastes_keep_pixels_selection_and_history() {
    let mut doc = Document::new(3, 2, [20, 30, 40, 255]).unwrap();
    let selection = Selection::elliptical(0, 0, 2, 2);
    doc.set_selection(Some(selection.clone()));
    assert_eq!(
        doc.paste_active_cancellable(1, 1, &[[1; 4]], &AtomicBool::new(true))
            .unwrap_err(),
        files::CANCELLED
    );
    assert!(
        doc.paste_active_cancellable(2, 2, &[[1; 4]], &AtomicBool::new(false))
            .is_err()
    );
    assert_eq!(doc.selection(), Some(selection));
    assert_eq!(doc.pixels(), &[[20, 30, 40, 255]; 6]);
    assert!(!doc.can_undo());
    assert!(!doc.dirty());
}
