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
