use paint_rust::{document::Document, transform};
use std::sync::atomic::AtomicBool;

#[test]
fn every_anchor_preserves_source_and_fills_only_new_pixels() {
    let source = [[10, 20, 30, 40], [50, 60, 70, 80]];
    let fill = [90, 100, 110, 120];
    for anchor in 0..9 {
        let output = transform::canvas_filled_cancellable(
            &source,
            (2, 1),
            (4, 3),
            anchor,
            fill,
            &AtomicBool::new(false),
        )
        .unwrap();
        let offset = anchor % 3 + (anchor / 3) * 4;
        for (i, pixel) in output.iter().enumerate() {
            assert_eq!(
                *pixel,
                if i == offset {
                    source[0]
                } else if i == offset + 1 {
                    source[1]
                } else {
                    fill
                }
            );
        }
        assert_eq!(
            transform::canvas_filled_cancellable(
                &output,
                (4, 3),
                (2, 1),
                anchor,
                fill,
                &AtomicBool::new(false)
            )
            .unwrap(),
            source
        );
    }
}

#[test]
fn filled_resize_cancels_atomically_and_undo_restores_hidden_layers() {
    let mut doc = Document::new(2, 1, [50, 60, 70, 80]).unwrap();
    doc.add_layer(true);
    doc.set_visibility(1, false);
    let before = doc.layers().to_vec();
    let history = doc.history().count();
    assert!(
        doc.resize_canvas_filled_cancellable(4, 3, 4, [90, 100, 110, 120], &AtomicBool::new(true))
            .is_err()
    );
    assert!(doc.layers() == before);
    assert_eq!(doc.history().count(), history);
    assert!(
        doc.resize_canvas_filled_cancellable(4, 3, 4, [90, 100, 110, 120], &AtomicBool::new(false))
            .unwrap()
    );
    assert_eq!(doc.history().count(), history + 1);
    assert!(!doc.layers()[1].visible);
    doc.undo();
    assert!(doc.layers() == before);
    assert_eq!((doc.width, doc.height), (2, 1));
}
