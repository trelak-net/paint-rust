use paint_rust::document::{Document, Selection};
use std::sync::atomic::AtomicBool;
#[test]
fn selection_undo_redo_interleaves_with_image_edits_and_saved_revisions() {
    let mut doc = Document::new(8, 8, [20, 30, 40, 255]).unwrap();
    let a = Selection::elliptical(1, 1, 4, 4);
    let b = Selection::elliptical(3, 3, 4, 4);
    assert!(doc.set_selection_with_history(Some(a.clone()), "Select"));
    assert!(!doc.dirty());
    doc.invert();
    let saved = doc.pixels().to_vec();
    doc.mark_saved();
    assert!(doc.set_selection_with_history(Some(b.clone()), "Select again"));
    assert!(!doc.dirty());
    assert!(doc.set_selection_with_history(None, "Deselect"));
    assert!(!doc.dirty());
    assert!(doc.undo());
    assert_eq!(doc.selection(), Some(b));
    assert!(!doc.dirty());
    assert!(doc.undo());
    assert_eq!(doc.selection(), Some(a.clone()));
    assert!(!doc.dirty());
    assert!(doc.undo());
    assert_eq!(doc.pixels(), &[[20, 30, 40, 255]; 64]);
    assert!(doc.dirty());
    assert!(doc.undo());
    assert!(doc.selection().is_none());
    assert!(doc.dirty());
    assert!(doc.redo());
    assert_eq!(doc.selection(), Some(a));
    assert!(doc.dirty());
    assert!(doc.redo());
    assert_eq!(doc.pixels(), saved);
    assert!(!doc.dirty());
}
#[test]
fn selection_noops_cancel_and_branching_preserve_state_without_false_dirty_flags() {
    let mut doc = Document::new(8, 8, [20, 30, 40, 255]).unwrap();
    let selection = Selection::elliptical(1, 1, 4, 4);
    assert!(!doc.set_selection_with_history(None, "No selection"));
    assert!(doc.set_selection_with_history(Some(selection.clone()), "Select"));
    assert!(!doc.set_selection_with_history(Some(selection.clone()), "No change"));
    assert_eq!(doc.history().count(), 1);
    doc.begin("Preview move");
    doc.preview_move(1, 1, false, true);
    doc.cancel();
    assert_eq!(doc.selection(), Some(selection.clone()));
    assert_eq!(doc.history().count(), 1);
    doc.undo();
    assert!(doc.can_redo());
    doc.set_selection_with_history(Some(Selection::elliptical(0, 0, 2, 2)), "New branch");
    assert!(!doc.can_redo());
    assert!(!doc.dirty());
}
#[test]
fn selection_history_survives_disk_storage_without_losing_masks_or_clean_revision() {
    let mut doc = Document::new(8, 8, [20, 30, 40, 255]).unwrap();
    doc.defer_history_storage(true);
    let a = Selection::elliptical(1, 1, 4, 4);
    let b = Selection::elliptical(3, 3, 4, 4);
    doc.set_selection_with_history(Some(a.clone()), "Select");
    doc.set_selection_with_history(Some(b.clone()), "Select again");
    doc.set_selection_with_history(None, "Deselect");
    assert!(doc.store_history_with_budget_cancellable(0, &AtomicBool::new(false)));
    assert_eq!(doc.history_bytes(), 0);
    assert!(doc.history_disk_bytes() > 0);
    assert!(doc.undo());
    assert_eq!(doc.selection(), Some(b));
    assert!(!doc.dirty());
    assert!(doc.undo());
    assert_eq!(doc.selection(), Some(a));
    assert!(!doc.dirty());
    assert!(doc.undo());
    assert!(doc.selection().is_none());
    assert!(!doc.dirty());
    for _ in 0..3 {
        assert!(doc.redo());
        assert!(!doc.dirty());
    }
    assert!(doc.selection().is_none());
    assert_eq!(doc.pixels(), &[[20, 30, 40, 255]; 64]);
}
