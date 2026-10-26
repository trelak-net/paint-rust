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

#[test]
#[ignore = "Requires static Open Sans and Noto Arabic variable fonts via Fontconfig"]
fn bold_primary_resolves_a_named_variable_fallback_and_renders_its_axes() {
    use paint_rust::text::{
        TextLayout, TextStyle, fallback::Resolver, render_with_fallback_cancellable,
    };
    let cancel = AtomicBool::new(false);
    let primary = paint_rust::text::load_font_cancellable(
        std::path::Path::new("/usr/share/fonts/open-sans/OpenSans-Bold.ttf"),
        &cancel,
    )
    .unwrap();
    let mut resolver = Resolver::default();
    assert!(resolver.prepare(&primary, 0, "مرحبا", &cancel));
    assert!(
        resolver
            .fonts()
            .iter()
            .any(|font| font.axes.get(*b"wght").is_some_and(|w| w >= 700.0))
    );
    let render = |faces| {
        let mut pixels = vec![[0; 4]; 400 * 120];
        assert!(render_with_fallback_cancellable(
            &mut pixels,
            400,
            120,
            None,
            TextStyle {
                layout: TextLayout::default(),
                font: &primary,
                text: "مرحبا",
                origin: (10.0, 10.0),
                size: 60.0,
                color: [255; 4],
            },
            faces,
            &cancel
        ));
        pixels
    };
    let heavy = render(resolver.fonts());
    let mut normal = resolver.fonts().to_vec();
    for font in &mut normal {
        font.axes = Default::default();
    }
    let normal = render(&normal);
    assert_ne!(heavy, normal);
    assert!(
        heavy.iter().map(|p| u64::from(p[3])).sum::<u64>()
            > normal.iter().map(|p| u64::from(p[3])).sum::<u64>()
    );
}

#[test]
#[ignore = "Requires Noto Sans and Noto Arabic variable fonts via Fontconfig"]
fn changing_primary_weight_axis_refreshes_and_updates_fallback_coordinates() {
    use paint_rust::text::{fallback::Resolver, variations::Settings};
    let cancel = AtomicBool::new(false);
    let primary = paint_rust::text::load_font_cancellable(
        std::path::Path::new("/usr/share/fonts/google-noto-vf/NotoSans[wght].ttf"),
        &cancel,
    )
    .unwrap();
    let mut resolver = Resolver::default();
    for weight in [700.0, 400.0, 650.0] {
        let mut axes = Settings::default();
        assert!(axes.set(*b"wght", weight));
        assert!(
            resolver.prepare_with_axes(&primary, 0, "مرحبا", axes, &cancel),
            "weight {weight}"
        );
        assert!(!resolver.fonts().is_empty());
        assert!(
            resolver
                .fonts()
                .iter()
                .any(|face| face.axes.get(*b"wght") == Some(weight))
        );
    }
    let mut invalid = Settings::default();
    invalid.set(*b"XXXX", 12.0);
    assert!(!resolver.prepare_with_axes(&primary, 0, "مرحبا", invalid, &cancel));
}
