use std::sync::atomic::AtomicBool;

#[test]
#[ignore = "Requires Fontconfig and installed desktop outline fonts"]
fn discover_and_load_real_system_font_faces() {
    let cancel = AtomicBool::new(false);
    let fonts = paint_rust::fonts::installed_cancellable(&cancel).unwrap();
    assert!(!fonts.is_empty());
    let mut loaded = 0;
    for font in fonts.iter().take(20) {
        if paint_rust::text::load_font_face_cancellable(&font.path, font.index, &cancel).is_ok() {
            loaded += 1;
        }
    }
    assert!(loaded > 0);
    println!(
        "Discovered {} faces; loaded {loaded} of the first 20",
        fonts.len()
    );
}
