use paint_rust::{
    brush::{Options, Paint},
    document::{Document, Layer},
};

fn stamp(document: &mut Document, point: (f32, f32)) {
    document.paint_segment(
        point,
        point,
        1.0,
        Paint::Clone { offset: (0, 0) },
        Options::default(),
    );
}

#[test]
fn clone_tracks_source_layer_through_reordering_and_targets_another_layer() {
    let mut document = Document::new(3, 2, [20, 40, 60, 180]).unwrap();
    let source_id = document.layers()[0].session_id();
    document.add_layer(false);
    let target_id = document.layers()[1].session_id();
    document.reorder_layer(0, 1);
    document.select_layer(0);
    assert_eq!(document.layers()[0].session_id(), target_id);
    document.prepare_clone_source(source_id, false).unwrap();
    document.begin("Clone");
    stamp(&mut document, (1.5, 0.5));
    document.commit();
    assert_eq!(document.pixels()[1], [20, 40, 60, 180]);
    assert_eq!(document.pixels()[0], [0; 4]);
    assert!(
        document.layers()[1]
            .pixels()
            .iter()
            .all(|p| *p == [20, 40, 60, 180])
    );
    document.undo();
    assert_eq!(document.pixels()[1], [0; 4]);
    document.redo();
    assert_eq!(document.pixels()[1], [20, 40, 60, 180]);
}

#[test]
fn merged_clone_uses_frozen_visible_composite_and_original_destination_coverage() {
    let bottom = Layer::new("Bottom".into(), vec![[80, 90, 100, 255]; 4]);
    let mut top = Layer::new("Top".into(), vec![[200, 40, 20, 128]; 4]);
    top.opacity = 100;
    let mut hidden = Layer::new("Hidden".into(), vec![[0, 255, 0, 255]; 4]);
    hidden.visible = false;
    let mut document = Document::from_layers(4, 1, vec![bottom, top, hidden], 1).unwrap();
    let source_id = document.layers()[1].session_id();
    let composite = document.composite();
    let original = document.pixels().to_vec();
    document.prepare_clone_source(source_id, true).unwrap();
    document.begin("Merged clone");
    stamp(&mut document, (0.5, 0.5));
    stamp(&mut document, (1.5, 0.5));
    assert_eq!(document.pixels()[0], composite[0]);
    assert_eq!(document.pixels()[1], composite[1]);
    assert_eq!(document.pixels()[2], original[2]);
    document.cancel();
    assert_eq!(document.pixels(), original);
}

#[test]
fn deleted_source_is_rejected_and_undo_restores_its_identity() {
    let mut document = Document::new(2, 2, [30; 4]).unwrap();
    let source_id = document.layers()[0].session_id();
    document.add_layer(true);
    assert_ne!(document.layers()[1].session_id(), source_id);
    document.select_layer(0);
    document.remove_layer();
    assert!(document.prepare_clone_source(source_id, false).is_err());
    document.undo();
    assert_eq!(document.layers()[0].session_id(), source_id);
    document.prepare_clone_source(source_id, false).unwrap();
}

#[test]
fn session_identity_does_not_change_image_equality() {
    let a = Layer::new("Layer".into(), vec![[0; 4]]);
    let b = Layer::new("Layer".into(), vec![[0; 4]]);
    assert_ne!(a.session_id(), b.session_id());
    assert!(a == b);
}
