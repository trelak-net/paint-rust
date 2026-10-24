use paint_rust::{document::Document, files};

#[test]
fn continuous_stroke_has_no_gaps_and_is_one_undo_step() {
    let mut doc = Document::new(32, 8, [255; 4]).unwrap();
    doc.begin("Brush");
    doc.stroke((1.5, 3.5), (30.5, 3.5), 1.0, [12, 34, 56, 255], false);
    assert!(doc.commit());
    for x in 1..=30 {
        assert_eq!(doc.sample(x, 3), Some([12, 34, 56, 255]));
    }
    assert_eq!(doc.history().count(), 1);
    doc.undo();
    assert!(doc.pixels().iter().all(|p| *p == [255; 4]));
    doc.redo();
    assert_eq!(doc.sample(15, 3), Some([12, 34, 56, 255]));
}

#[test]
fn cancel_restores_pixels_and_preserves_redo() {
    let mut doc = Document::new(8, 8, [255; 4]).unwrap();
    doc.invert();
    doc.undo();
    doc.begin("Eraser");
    doc.stroke((0.0, 0.0), (8.0, 8.0), 3.0, [0; 4], true);
    assert_eq!(doc.sample(4, 4), Some([0; 4]));
    doc.cancel();
    assert!(!doc.dirty());
    assert!(doc.can_redo());
    assert!(doc.pixels().iter().all(|p| *p == [255; 4]));
}

#[test]
fn saved_revision_tracks_undo_redo_and_branches() {
    let mut doc = Document::new(8, 8, [255; 4]).unwrap();
    doc.invert();
    doc.mark_saved();
    assert!(!doc.dirty());
    doc.undo();
    assert!(doc.dirty());
    doc.redo();
    assert!(!doc.dirty());
    doc.undo();
    doc.invert();
    assert!(doc.dirty());
    assert!(!doc.can_redo());
}

#[test]
fn fill_respects_boundaries_and_noop_keeps_history_clean() {
    let mut doc = Document::new(9, 9, [255; 4]).unwrap();
    doc.begin("Boundary");
    doc.stroke((4.5, 0.5), (4.5, 8.5), 1.0, [0, 0, 0, 255], false);
    doc.commit();
    doc.begin("Fill");
    doc.fill(0, 0, [255, 0, 0, 255]);
    doc.commit();
    assert_eq!(doc.sample(3, 8), Some([255, 0, 0, 255]));
    assert_eq!(doc.sample(5, 8), Some([255; 4]));
    doc.begin("No change");
    doc.fill(0, 0, [255, 0, 0, 255]);
    assert!(!doc.commit());
    assert_eq!(doc.history().count(), 2);
}

#[test]
fn invalid_dimensions_and_buffers_are_rejected() {
    assert!(Document::new(0, 5, [0; 4]).is_err());
    assert!(Document::new(u32::MAX, u32::MAX, [0; 4]).is_err());
    assert!(Document::from_pixels(5, 5, vec![]).is_err());
}

#[test]
fn png_roundtrip_preserves_rgba_and_replaces_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("image.png");
    std::fs::write(&path, b"previous contents").unwrap();
    let doc = Document::from_pixels(
        2,
        2,
        vec![[1, 2, 3, 0], [40, 60, 80, 128], [255; 4], [0, 0, 0, 255]],
    )
    .unwrap();
    files::save_png(&doc, &path).unwrap();
    let reopened = files::open(&path).unwrap();
    assert_eq!(reopened.pixels(), doc.pixels());
    assert_eq!((reopened.width, reopened.height), (2, 2));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn failed_save_does_not_replace_destination_or_leave_temp_files() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("directory.png");
    std::fs::create_dir(&target).unwrap();
    let doc = Document::new(2, 2, [255; 4]).unwrap();
    assert!(files::save_png(&doc, &target).is_err());
    assert!(target.is_dir());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn corrupt_input_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.png");
    std::fs::write(&path, b"not an image").unwrap();
    assert!(files::open(&path).is_err());
}

#[test]
fn history_evicts_oldest_snapshots_at_the_memory_budget() {
    let mut doc = Document::new(512, 512, [255; 4]).unwrap();
    for _ in 0..130 {
        doc.invert();
    }
    assert_eq!(doc.history().count(), 128);
    assert_eq!(doc.history_bytes(), 128 * 1024 * 1024);
    for _ in 0..128 {
        doc.undo();
    }
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
    assert!(doc.history_bytes() <= 128 * 1024 * 1024);
}

#[test]
fn readonly_destination_is_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("locked.png");
    std::fs::write(&path, b"keep these bytes").unwrap();
    let original = std::fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&path, readonly).unwrap();
    let doc = Document::new(2, 2, [255; 4]).unwrap();
    assert!(files::save_png(&doc, &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"keep these bytes");
    std::fs::set_permissions(&path, original).unwrap();
}
