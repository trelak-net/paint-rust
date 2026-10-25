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

#[test]
#[ignore = "Requires Fontconfig and installed Arabic outline fonts"]
fn missing_script_resolves_off_thread_and_reuses_loaded_fonts() {
    use ab_glyph::Font;
    use paint_rust::text::fallback::Resolver;
    let definitions = eframe::egui::FontDefinitions::default();
    let primary =
        ab_glyph::FontArc::try_from_vec(definitions.font_data["Ubuntu-Light"].font.to_vec())
            .unwrap();
    let mut resolver = Resolver::default();
    let cancel = AtomicBool::new(false);
    assert!(resolver.prepare(&primary, 0, "Hello", &cancel));
    assert!(resolver.fonts().is_empty());
    assert!(resolver.prepare(&primary, 0, "مرحبا", &cancel));
    assert!(!resolver.fonts().is_empty());
    for ch in "مرحبا".chars() {
        assert!(
            resolver
                .fonts()
                .iter()
                .any(|font| font.font.glyph_id(ch).0 != 0)
        );
    }
    let count = resolver.fonts().len();
    assert!(resolver.prepare(&primary, 0, "مرحبا", &cancel));
    assert_eq!(resolver.fonts().len(), count);
    assert!(resolver.prepare(&primary, 0, "مرحبا שלום", &cancel));
    for ch in "שלום".chars() {
        assert!(
            resolver
                .fonts()
                .iter()
                .any(|font| font.font.glyph_id(ch).0 != 0)
        );
    }
    assert!(!resolver.prepare(&primary, 0, "مرحبا", &AtomicBool::new(true)));
}
