use paint_rust::{document::Document, files, recovery::Recovery};

fn wait(recovery: &mut Recovery) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while recovery.busy() {
        recovery.poll().unwrap();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn recovery_snapshot_is_immutable_and_live_sessions_are_not_offered() {
    let dir = tempfile::tempdir().unwrap();
    let mut recovery = Recovery::new(dir.path()).unwrap();
    let mut doc = Document::new(16, 16, [255; 4]).unwrap();
    doc.invert();
    assert!(recovery.schedule(&doc, || {}).unwrap());
    doc.invert();
    wait(&mut recovery);
    assert!(Recovery::discover(dir.path()).unwrap().is_empty());
    drop(recovery);
    let candidates = Recovery::discover(dir.path()).unwrap();
    assert_eq!(candidates.len(), 1);
    let loaded = files::open(&candidates[0].path).unwrap();
    assert_eq!(loaded.pixels()[0], [0, 0, 0, 255]);
    assert_eq!(doc.pixels()[0], [255; 4]);
}

#[test]
fn clearing_a_pending_snapshot_never_resurrects_discarded_work() {
    let dir = tempfile::tempdir().unwrap();
    let mut recovery = Recovery::new(dir.path()).unwrap();
    let mut doc = Document::new(256, 256, [255; 4]).unwrap();
    doc.invert();
    recovery.schedule(&doc, || {}).unwrap();
    recovery.clear().unwrap();
    wait(&mut recovery);
    drop(recovery);
    assert!(Recovery::discover(dir.path()).unwrap().is_empty());
}

#[test]
fn a_new_edit_after_save_gets_a_new_recoverable_epoch() {
    let dir = tempfile::tempdir().unwrap();
    let mut recovery = Recovery::new(dir.path()).unwrap();
    let mut doc = Document::new(8, 8, [255; 4]).unwrap();
    doc.invert();
    recovery.schedule(&doc, || {}).unwrap();
    wait(&mut recovery);
    recovery.clear().unwrap();
    doc.invert();
    recovery.schedule(&doc, || {}).unwrap();
    wait(&mut recovery);
    drop(recovery);
    let candidates = Recovery::discover(dir.path()).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        files::open(&candidates[0].path).unwrap().pixels()[0],
        [255; 4]
    );
}

#[test]
fn recovered_work_remains_available_until_a_replacement_snapshot_is_safe() {
    let dir = tempfile::tempdir().unwrap();
    let mut first = Recovery::new(dir.path()).unwrap();
    let mut doc = Document::new(8, 8, [255; 4]).unwrap();
    doc.invert();
    first.schedule(&doc, || {}).unwrap();
    wait(&mut first);
    drop(first);
    let candidate = Recovery::discover(dir.path()).unwrap().remove(0);
    let old_path = candidate.path.clone();
    let mut recovered = files::open(&candidate.path).unwrap();
    recovered.mark_unsaved();
    assert!(recovered.dirty());
    let mut next = Recovery::new(dir.path()).unwrap();
    next.adopt(candidate);
    assert!(old_path.exists());
    next.schedule(&recovered, || {}).unwrap();
    wait(&mut next);
    assert!(!old_path.exists());
    drop(next);
    assert_eq!(Recovery::discover(dir.path()).unwrap().len(), 1);
}

#[test]
fn uncommitted_previews_are_not_autosaved() {
    let dir = tempfile::tempdir().unwrap();
    let mut recovery = Recovery::new(dir.path()).unwrap();
    let mut doc = Document::new(8, 8, [255; 4]).unwrap();
    doc.begin("Stroke");
    doc.fill(0, 0, [0; 4]);
    assert!(!recovery.schedule(&doc, || {}).unwrap());
    drop(recovery);
    assert!(Recovery::discover(dir.path()).unwrap().is_empty());
}
