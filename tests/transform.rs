use paint_rust::{
    document::{Document, Selection},
    transform::Orientation,
};

fn numbered() -> Document {
    Document::from_pixels(3, 2, (1..=6).map(|n| [n, 0, 0, 255]).collect()).unwrap()
}

#[test]
fn rotation_and_mirror_preserve_pixels_and_layer_properties() {
    let mut doc = numbered();
    doc.add_layer(true);
    doc.set_layer_properties("Upper".into(), 100);
    doc.reorient(Orientation::Clockwise).unwrap();
    assert_eq!((doc.width, doc.height), (2, 3));
    assert_eq!(
        doc.pixels().iter().map(|p| p[0]).collect::<Vec<_>>(),
        vec![4, 1, 5, 2, 6, 3]
    );
    assert_eq!(doc.layers()[1].opacity, 100);
    doc.reorient(Orientation::Counterclockwise).unwrap();
    assert_eq!(doc.pixels(), numbered().pixels());
    doc.reorient(Orientation::FlipHorizontal).unwrap();
    assert_eq!(
        doc.pixels().iter().map(|p| p[0]).collect::<Vec<_>>(),
        vec![3, 2, 1, 6, 5, 4]
    );
    doc.undo();
    assert_eq!(doc.pixels(), numbered().pixels());
    doc.reorient(Orientation::FlipVertical).unwrap();
    assert_eq!(
        doc.pixels().iter().map(|p| p[0]).collect::<Vec<_>>(),
        vec![4, 5, 6, 1, 2, 3]
    );
}

#[test]
fn canvas_anchor_expands_with_transparency_and_crops_without_resampling() {
    let mut doc = numbered();
    doc.resize_canvas(5, 4, 4).unwrap();
    assert_eq!(doc.pixels()[0], [0; 4]);
    assert_eq!(doc.pixels()[6], [1, 0, 0, 255]);
    assert_eq!(doc.pixels()[2 * 5 + 3], [6, 0, 0, 255]);
    doc.resize_canvas(3, 2, 4).unwrap();
    assert_eq!(doc.pixels(), numbered().pixels());
    doc.undo();
    assert_eq!((doc.width, doc.height), (5, 4));
}

#[test]
fn smooth_resize_uses_premultiplied_alpha_to_avoid_transparent_color_bleed() {
    let mut doc = Document::from_pixels(2, 1, vec![[255, 0, 0, 255], [0, 0, 255, 0]]).unwrap();
    doc.resize_image(3, 1, true).unwrap();
    assert_eq!(doc.pixels()[1], [255, 0, 0, 128]);
    doc.undo();
    assert_eq!(doc.pixels()[1], [0, 0, 255, 0]);
    doc.resize_image(4, 1, false).unwrap();
    assert_eq!(
        doc.pixels(),
        &[
            [255, 0, 0, 255],
            [255, 0, 0, 255],
            [0, 0, 255, 0],
            [0, 0, 255, 0]
        ]
    );
}

#[test]
fn invalid_resize_is_transactional_and_undo_restores_selection() {
    let mut doc = numbered();
    let selection = Selection {
        x: 1,
        y: 0,
        width: 2,
        height: 2,
    };
    doc.set_selection(Some(selection));
    assert!(doc.resize_image(0, 1, true).is_err());
    assert!(!doc.dirty());
    assert!(!doc.can_undo());
    assert!(doc.resize_canvas(2, 2, 9).is_err());
    doc.resize_image(6, 4, false).unwrap();
    assert_eq!(doc.selection(), None);
    doc.undo();
    assert_eq!(doc.selection(), Some(selection));
    assert_eq!((doc.width, doc.height), (3, 2));
    assert!(!doc.dirty());
    doc.redo();
    assert_eq!((doc.width, doc.height), (6, 4));
}

#[test]
fn publishing_background_geometry_retains_history_and_saved_state() {
    let mut doc = Document::new(3, 2, [10, 20, 30, 255]).unwrap();
    doc.invert();
    doc.mark_saved();
    doc.set_selection(Some(Selection {
        x: 1,
        y: 0,
        width: 2,
        height: 2,
    }));
    let selection = doc.selection();
    let original = doc.pixels().to_vec();
    let mut computed = doc.snapshot();
    computed.resize_image(6, 4, false).unwrap();
    assert!(doc.apply_transformed(computed, "Resize image").unwrap());
    assert_eq!((doc.width, doc.height), (6, 4));
    assert!(doc.selection().is_none());
    assert!(doc.dirty());
    doc.undo();
    assert_eq!(doc.pixels(), original);
    assert_eq!(doc.selection(), selection);
    assert!(!doc.dirty());
    doc.undo();
    assert_eq!(doc.pixels(), &[[10, 20, 30, 255]; 6]);
    doc.redo();
    doc.redo();
    assert_eq!((doc.width, doc.height), (6, 4));
}

#[test]
fn unchanged_background_geometry_preserves_selection_and_pending_edit_is_rejected() {
    let mut doc = Document::new(2, 2, [255; 4]).unwrap();
    let selection = Some(Selection {
        x: 1,
        y: 0,
        width: 1,
        height: 1,
    });
    doc.set_selection(selection);
    assert!(!doc.apply_transformed(doc.snapshot(), "Unchanged").unwrap());
    assert_eq!(doc.selection(), selection);
    assert!(!doc.can_undo());
    let mut computed = doc.snapshot();
    computed.resize_image(4, 4, false).unwrap();
    doc.begin("Stroke");
    assert!(doc.apply_transformed(computed, "Resize").is_err());
    assert_eq!((doc.width, doc.height), (2, 2));
    assert!(doc.editing());
}
