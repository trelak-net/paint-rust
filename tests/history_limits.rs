use paint_rust::document::Document;
use std::sync::atomic::AtomicBool;

fn edited() -> Document {
    let mut state = 123456789u32;
    let pixels = (0..4096)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state.to_le_bytes()
        })
        .collect();
    let mut document = Document::from_pixels(64, 64, pixels).unwrap();
    document.defer_history_storage(true);
    for _ in 0..3 {
        document.invert();
    }
    document
}
#[test]
fn disk_limit_keeps_unstored_history_and_full_undo_redo() {
    let cancel = AtomicBool::new(false);
    let mut document = edited();
    let original = document.pixels().to_vec();
    assert!(document.store_history_with_limits_cancellable(0, 20_000, &cancel));
    assert!(document.history_disk_bytes() > 0);
    assert!(document.history_disk_bytes() <= 20_000);
    assert!(document.history_bytes() > 0);
    assert!(document.history_error().unwrap().contains("disk budget"));
    assert_eq!(document.history().count(), 3);
    for _ in 0..3 {
        assert!(document.undo());
    }
    assert!(!document.dirty());
    for _ in 0..3 {
        assert!(document.redo());
    }
    assert_eq!(document.pixels(), original);
    assert!(document.store_history_with_limits_cancellable(0, 1_000_000, &cancel));
    assert!(document.history_error().is_none());
    assert_eq!(document.history_bytes(), 0);
    for _ in 0..3 {
        assert!(document.undo());
    }
    assert!(!document.dirty());
}
#[test]
fn zero_disk_budget_does_not_destroy_history_or_create_payloads() {
    let mut document = edited();
    let before = document.history_bytes();
    assert!(document.store_history_with_limits_cancellable(0, 0, &AtomicBool::new(false)));
    assert_eq!(document.history_bytes(), before);
    assert_eq!(document.history_disk_bytes(), 0);
    assert_eq!(document.history().count(), 3);
    assert!(document.history_error().is_some());
    assert!(!document.store_history_with_limits_cancellable(0, 1_000_000, &AtomicBool::new(true)));
    assert_eq!(document.history_bytes(), before);
}
