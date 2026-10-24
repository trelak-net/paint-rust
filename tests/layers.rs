use paint_rust::{document::Document, files};

#[test]
fn layers_composite_with_opacity_visibility_and_order() {
    let mut doc = Document::new(2, 2, [0, 0, 255, 255]).unwrap();
    doc.add_layer(false);
    doc.begin("Paint");
    doc.fill(0, 0, [255, 0, 0, 128]);
    doc.commit();
    assert_eq!(doc.composite()[0], [128, 0, 127, 255]);
    doc.set_layer_properties("Overlay".into(), 128);
    assert_eq!(doc.composite()[0], [64, 0, 191, 255]);
    doc.set_visibility(1, false);
    assert_eq!(doc.composite()[0], [0, 0, 255, 255]);
    doc.set_visibility(1, true);
    doc.move_layer(false);
    assert_eq!(doc.composite()[0], [0, 0, 255, 255]);
    doc.undo();
    assert_eq!(doc.composite()[0], [64, 0, 191, 255]);
    assert_eq!(doc.active_layer(), 1);
}

#[test]
fn duplicate_uses_independent_edits_and_undo_preserves_both_layers() {
    let mut doc = Document::new(4, 4, [10, 20, 30, 255]).unwrap();
    doc.add_layer(true);
    doc.invert();
    assert_eq!(doc.layers()[0].pixels()[0], [10, 20, 30, 255]);
    assert_eq!(doc.layers()[1].pixels()[0], [245, 235, 225, 255]);
    doc.undo();
    assert_eq!(doc.layers()[0].pixels(), doc.layers()[1].pixels());
    doc.redo();
    assert_ne!(doc.layers()[0].pixels(), doc.layers()[1].pixels());
    doc.remove_layer();
    assert_eq!(doc.layers().len(), 1);
    doc.undo();
    assert_eq!(doc.layers().len(), 2);
    assert_eq!(doc.active_layer(), 1);
}

#[test]
fn selecting_layers_is_not_a_document_edit_and_properties_share_buffers() {
    let mut doc = Document::new(16, 16, [255; 4]).unwrap();
    doc.add_layer(false);
    doc.mark_saved();
    let before = doc.history().count();
    doc.select_layer(0);
    assert!(!doc.dirty());
    assert_eq!(doc.history().count(), before);
    for opacity in 0..100 {
        doc.set_layer_properties("Background".into(), opacity);
    }
    assert!(doc.history_bytes() <= 2 * 16 * 16 * 4);
    assert!(!doc.remove_layer() || doc.layers().len() == 1);
    assert!(!doc.remove_layer());
}

#[test]
fn project_roundtrip_preserves_all_layer_data_and_png_only_flattens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("painting.prust");
    let mut doc = Document::new(3, 2, [12, 24, 36, 255]).unwrap();
    doc.add_layer(false);
    doc.begin("Fill");
    doc.fill(0, 0, [200, 100, 50, 100]);
    doc.commit();
    doc.set_layer_properties("Zażółć 🎨".into(), 144);
    doc.add_layer(true);
    doc.set_visibility(2, false);
    doc.select_layer(1);
    files::save_project(&doc, &path).unwrap();
    let loaded = files::open(&path).unwrap();
    assert_eq!(
        (loaded.width, loaded.height, loaded.active_layer()),
        (3, 2, 1)
    );
    assert_eq!(loaded.composite(), doc.composite());
    for (a, b) in loaded.layers().iter().zip(doc.layers()) {
        assert_eq!(
            (&a.name, a.visible, a.opacity, a.pixels()),
            (&b.name, b.visible, b.opacity, b.pixels())
        );
    }
    assert!(!loaded.dirty());
    assert!(!loaded.can_undo());
    let png = dir.path().join("export.png");
    files::save_png(&doc, &png).unwrap();
    assert_eq!(files::open(&png).unwrap().pixels(), doc.composite());
    assert!(doc.dirty());
}

#[test]
fn invalid_and_truncated_projects_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.prust");
    let doc = Document::new(2, 2, [255; 4]).unwrap();
    files::save_project(&doc, &path).unwrap();
    let valid = std::fs::read(&path).unwrap();
    for length in [0, 7, 20, valid.len() - 1] {
        std::fs::write(&path, &valid[..length]).unwrap();
        assert!(files::open(&path).is_err());
    }
    for (offset, value) in [(6, 99), (16, 0), (16, 33), (20, 9)] {
        let mut invalid = valid.clone();
        invalid[offset] = value;
        std::fs::write(&path, &invalid).unwrap();
        assert!(files::open(&path).is_err());
    }
    let mut trailing = valid;
    trailing.push(0);
    std::fs::write(&path, trailing).unwrap();
    assert!(files::open(&path).is_err());
}

#[test]
fn flatten_preserves_composite_and_undo_restores_every_layer() {
    use paint_rust::document::{BlendMode, Selection};
    for mode in BlendMode::ALL {
        let mut doc = Document::new(3, 2, [20, 50, 90, 123]).unwrap();
        doc.add_layer(false);
        doc.begin("Paint");
        doc.fill(0, 0, [200, 130, 80, 180]);
        doc.commit();
        doc.set_layer_style("Overlay".into(), 169, mode);
        doc.add_layer(true);
        doc.set_visibility(2, false);
        doc.select_layer(1);
        let selection = Selection {
            x: 1,
            y: 0,
            width: 1,
            height: 1,
        };
        doc.set_selection(Some(selection));
        doc.mark_saved();
        let layers = doc.layers().to_vec();
        let composite = doc.composite();
        assert!(doc.flatten());
        assert_eq!(doc.layers().len(), 1);
        assert_eq!(doc.composite(), composite);
        assert_eq!(doc.selection(), Some(selection));
        assert!(doc.dirty());
        doc.undo();
        assert!(doc.layers() == layers);
        assert_eq!(doc.active_layer(), 1);
        assert!(!doc.dirty());
        doc.redo();
        assert_eq!(doc.composite(), composite);
        assert_eq!(doc.layers().len(), 1);
    }
}

#[test]
fn flatten_hidden_or_translucent_single_layer_bakes_visibility_and_opacity() {
    let mut doc = Document::new(1, 1, [200, 100, 30, 160]).unwrap();
    doc.set_layer_properties("Translucent".into(), 128);
    let composite = doc.composite();
    assert!(doc.flatten());
    assert_eq!(doc.pixels(), composite);
    assert_eq!(doc.layers()[0].opacity, 255);
    doc.undo();
    doc.set_visibility(0, false);
    assert!(doc.flatten());
    assert_eq!(doc.pixels(), &[[0; 4]]);
}

#[test]
fn flatten_is_noop_for_plain_raster_and_refuses_pending_edits() {
    let mut doc = Document::new(1, 1, [23, 45, 67, 0]).unwrap();
    assert!(!doc.flatten());
    assert!(!doc.dirty());
    assert!(!doc.can_undo());
    doc.add_layer(false);
    doc.begin("Preview");
    assert!(!doc.flatten());
    assert!(doc.editing());
    assert_eq!(doc.layers().len(), 2);
    doc.cancel();
    assert!(doc.flatten());
    assert_eq!(doc.pixels(), &[[23, 45, 67, 0]]);
}

#[test]
fn import_clips_at_selection_origin_and_rejects_invalid_input_atomically() {
    use paint_rust::document::Selection;
    let mut doc = Document::new(3, 2, [255; 4]).unwrap();
    let selection = Selection {
        x: 2,
        y: 1,
        width: 1,
        height: 1,
    };
    doc.set_selection(Some(selection));
    assert!(
        doc.import_layer(2, 2, &[[23, 45, 67, 89]; 4], "Detail")
            .unwrap()
    );
    assert_eq!(doc.pixels()[5], [23, 45, 67, 89]);
    assert!(doc.pixels()[..5].iter().all(|p| *p == [0; 4]));
    assert_eq!(doc.history().next(), Some("Import image layer"));
    doc.undo();
    assert_eq!(doc.selection(), Some(selection));
    assert!(!doc.dirty());
    assert!(doc.import_layer(2, 2, &[], "Invalid").is_err());
    assert!(
        doc.import_layer(1, 1, &[[0; 4]], &"x".repeat(4097))
            .is_err()
    );
    assert!(!doc.dirty());
    assert_eq!(doc.layers().len(), 1);
}
