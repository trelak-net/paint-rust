use paint_rust::document::{BlendMode, Document, Layer, Selection};
use std::sync::atomic::AtomicBool;
fn source() -> Document {
    let mut lower = Layer::new("Hidden source".into(), vec![[10, 20, 30, 40]; 4]);
    lower.visible = false;
    lower.is_background = true;
    lower.metadata.insert("note".into(), "preserve".into());
    let mut upper = Layer::new("Detail".into(), vec![[50, 60, 70, 80]; 4]);
    upper.opacity = 123;
    upper.blend_mode = BlendMode::Multiply;
    Document::from_layers(2, 2, vec![lower, upper], 1).unwrap()
}
#[test]
fn stack_import_preserves_properties_order_identity_and_clean_undo() {
    let source = source();
    let mut target = Document::new(5, 4, [255; 4]).unwrap();
    target.add_layer(false);
    target.add_layer(false);
    target.select_layer(1);
    target.mark_saved();
    let old = target.layers().to_vec();
    let history = target.history().count();
    assert!(
        target
            .import_layers_cancellable(&source, &AtomicBool::new(false))
            .unwrap()
    );
    assert_eq!(target.layers().len(), 5);
    assert_eq!(target.active_layer(), 3);
    assert!(
        target.layers()[0] == old[0]
            && target.layers()[1] == old[1]
            && target.layers()[4] == old[2]
    );
    for (index, original) in source.layers().iter().enumerate() {
        let imported = &target.layers()[index + 2];
        assert_ne!(imported.session_id(), original.session_id());
        assert_eq!(imported.name, original.name);
        assert_eq!(imported.metadata, original.metadata);
        assert_eq!(imported.is_background, original.is_background);
        assert_eq!(imported.visible, original.visible);
        assert_eq!(imported.opacity, original.opacity);
        assert_eq!(imported.blend_mode, original.blend_mode);
        for y in 0..4 {
            for x in 0..5 {
                let expected = if (1..3).contains(&x) && (1..3).contains(&y) {
                    original.pixels()[0]
                } else {
                    [0; 4]
                };
                assert_eq!(imported.pixels()[y * 5 + x], expected);
            }
        }
    }
    assert_eq!(target.history().count(), history + 1);
    assert!(target.undo());
    assert!(target.layers() == old);
    assert_eq!(target.active_layer(), 1);
    assert!(!target.dirty());
    assert!(target.redo());
    assert_eq!(target.layers().len(), 5);
}
#[test]
fn import_at_selection_origin_clips_and_preserves_destination_selection_and_resolution() {
    let mut target = Document::new(5, 4, [255; 4]).unwrap();
    target
        .set_resolution(paint_rust::resolution::Dpi { x: 144.0, y: 72.0 })
        .unwrap();
    let selection = Selection::elliptical(4, 3, 1, 1);
    target.set_selection(Some(selection.clone()));
    target
        .import_layers_cancellable(&source(), &AtomicBool::new(false))
        .unwrap();
    assert_eq!(target.selection(), Some(selection));
    assert_eq!(target.resolution().x, 144.0);
    assert_eq!(target.resolution().y, 72.0);
    assert_eq!(target.pixels()[19], [50, 60, 70, 80]);
    assert!(target.pixels()[..19].iter().all(|p| *p == [0; 4]));
}
#[test]
fn cancelled_and_over_limit_imports_leave_target_unchanged() {
    let mut target = Document::new(2, 2, [255; 4]).unwrap();
    assert!(
        target
            .import_layers_cancellable(&source(), &AtomicBool::new(true))
            .is_err()
    );
    assert_eq!(target.layers().len(), 1);
    assert!(!target.can_undo());
    while target.layers().len() < 31 {
        assert!(target.add_layer(false));
    }
    let before = target.layers().to_vec();
    let history = target.history().count();
    assert!(
        target
            .import_layers_cancellable(&source(), &AtomicBool::new(false))
            .is_err()
    );
    assert!(target.layers() == before);
    assert_eq!(target.history().count(), history);
}
#[test]
fn equal_size_import_shares_immutable_pixel_buffers_but_not_layer_identity() {
    let source = source();
    let mut target = Document::new(2, 2, [255; 4]).unwrap();
    target
        .import_layers_cancellable(&source, &AtomicBool::new(false))
        .unwrap();
    for (original, imported) in source.layers().iter().zip(&target.layers()[1..]) {
        assert_eq!(original.pixels().as_ptr(), imported.pixels().as_ptr());
        assert_ne!(original.session_id(), imported.session_id());
    }
}
