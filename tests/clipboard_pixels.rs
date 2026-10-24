use paint_rust::document::{Document, Selection};

#[test]
fn copy_reads_selection_from_active_or_merged_layers() {
    let mut doc = Document::new(4, 4, [50, 60, 70, 255]).unwrap();
    doc.add_layer(false);
    doc.set_selection(Some(Selection {
        mask: None,
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
        mask: None,
        ellipse: None,
        x: 3,
        y: 2,
        width: 1,
        height: 1,
    };
    doc.set_selection(Some(before.clone()));
    doc.paste_layer(2, 2, &[[255; 4]; 4]).unwrap();
    assert_eq!(
        doc.selection(),
        Some(Selection {
            mask: None,
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

#[test]
fn regional_merged_copy_matches_full_composite_for_every_blend_mode() {
    use paint_rust::document::{BlendMode, Layer};
    for mode in BlendMode::ALL {
        let background = Layer::new(
            "Background".into(),
            (0..64).map(|i| [i * 3, 70, 150, 200]).collect(),
        );
        let mut foreground = Layer::new("Foreground".into(), vec![[220, 120, 30, 170]; 64]);
        foreground.blend_mode = mode;
        foreground.opacity = 123;
        let mut doc = Document::from_layers(8, 8, vec![background, foreground], 1).unwrap();
        doc.set_selection(Some(Selection {
            x: 3,
            y: 2,
            width: 2,
            height: 3,
            mask: None,
            ellipse: None,
        }));
        let composite = doc.composite();
        let (_, _, pixels) = doc.copy_pixels(true);
        let expected: Vec<_> = (2..5)
            .flat_map(|y| composite[y * 8 + 3..y * 8 + 5].iter().copied())
            .collect();
        assert_eq!(pixels, expected);
    }
}

#[test]
fn cancelled_copy_preserves_source_and_returns_no_pixels() {
    let doc = Document::new(4, 4, [17; 4]).unwrap();
    for merged in [false, true] {
        assert!(
            doc.copy_pixels_cancellable(merged, &std::sync::atomic::AtomicBool::new(true))
                .is_none()
        );
    }
    assert_eq!(doc.pixels(), &[[17; 4]; 16]);
    assert!(!doc.dirty());
}
