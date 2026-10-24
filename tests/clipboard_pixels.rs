use paint_rust::document::{Document, Selection};

#[test]
fn copy_reads_selection_from_active_or_merged_layers() {
    let mut doc = Document::new(4, 4, [50, 60, 70, 255]).unwrap();
    doc.add_layer(false);
    doc.set_selection(Some(Selection {
        ellipse: None,
        x: 1,
        y: 1,
        width: 2,
        height: 2,
    }));
    let (width, height, raw) = doc.copy_pixels(false);
    assert_eq!((width, height), (2, 2));
    assert!(raw.iter().all(|p| *p == [0; 4]));
    let (_, _, merged) = doc.copy_pixels(true);
    assert!(merged.iter().all(|p| *p == [50, 60, 70, 255]));
    assert_eq!(doc.history().count(), 1);
}

#[test]
fn paste_clips_at_selection_origin_and_restores_previous_selection_on_undo() {
    let mut doc = Document::new(4, 4, [0; 4]).unwrap();
    let before = Selection {
        ellipse: None,
        x: 3,
        y: 2,
        width: 1,
        height: 1,
    };
    doc.set_selection(Some(before));
    doc.paste_layer(2, 2, &[[255; 4]; 4]).unwrap();
    assert_eq!(
        doc.selection(),
        Some(Selection {
            ellipse: None,
            x: 3,
            y: 2,
            width: 1,
            height: 2
        })
    );
    assert_eq!(doc.layers().len(), 2);
    assert_eq!(doc.history().count(), 1);
    assert_eq!(doc.pixels().iter().filter(|p| **p == [255; 4]).count(), 2);
    doc.undo();
    assert_eq!(doc.layers().len(), 1);
    assert_eq!(doc.selection(), Some(before));
    assert!(!doc.dirty());
    doc.redo();
    assert_eq!(doc.layers().len(), 2);
}

#[test]
fn invalid_paste_leaves_document_and_history_unchanged() {
    let mut doc = Document::new(4, 4, [255; 4]).unwrap();
    assert!(doc.paste_layer(2, 2, &[[0; 4]]).is_err());
    assert!(!doc.dirty());
    assert!(!doc.can_undo());
    assert_eq!(doc.layers().len(), 1);
}
