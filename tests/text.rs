use paint_rust::{
    document::{Document, Selection},
    text::TextStyle,
};

fn font() -> ab_glyph::FontArc {
    let definitions = eframe::egui::FontDefinitions::default();
    let name = &definitions.families[&eframe::egui::FontFamily::Proportional][0];
    ab_glyph::FontArc::try_from_vec(definitions.font_data[name].font.to_vec()).unwrap()
}

#[test]
fn text_preview_supports_unicode_and_replaces_previous_content() {
    let font = font();
    let mut doc = Document::new(200, 100, [0; 4]).unwrap();
    doc.begin("Text");
    doc.preview_text(TextStyle {
        layout: Default::default(),
        font: &font,
        text: "Żółć\nRust",
        origin: (3.0, 3.0),
        size: 28.0,
        color: [80, 120, 220, 255],
    })
    .unwrap();
    assert!(doc.pixels().iter().any(|p| p[3] > 0));
    assert!(doc.pixels().iter().any(|p| p[3] > 0 && p[3] < 255));
    doc.preview_text(TextStyle {
        layout: Default::default(),
        font: &font,
        text: "",
        origin: (3.0, 3.0),
        size: 28.0,
        color: [80, 120, 220, 255],
    })
    .unwrap();
    assert!(doc.pixels().iter().all(|p| *p == [0; 4]));
    assert!(!doc.commit());
}

#[test]
fn text_clips_to_selection_and_commits_once_on_the_active_layer() {
    let font = font();
    let mut doc = Document::new(100, 60, [255; 4]).unwrap();
    doc.add_layer(false);
    doc.set_selection(Some(Selection {
        mask: None,
        ellipse: None,
        x: 0,
        y: 0,
        width: 25,
        height: 60,
    }));
    doc.begin("Text");
    doc.preview_text(TextStyle {
        layout: Default::default(),
        font: &font,
        text: "Wide text",
        origin: (0.0, 0.0),
        size: 32.0,
        color: [255, 0, 0, 180],
    })
    .unwrap();
    assert!(doc.commit());
    assert_eq!(doc.history().count(), 2);
    assert!(doc.layers()[0].pixels().iter().all(|p| *p == [255; 4]));
    assert!(
        doc.pixels()
            .iter()
            .enumerate()
            .all(|(i, p)| i % 100 < 25 || *p == [0; 4])
    );
    doc.undo();
    assert!(doc.pixels().iter().all(|p| *p == [0; 4]));
    doc.redo();
    assert!(doc.pixels().iter().any(|p| p[3] > 0));
}

#[test]
fn invalid_text_parameters_and_cancellation_preserve_pixels() {
    let font = font();
    let mut doc = Document::new(100, 60, [255; 4]).unwrap();
    doc.begin("Text");
    assert!(
        doc.preview_text(TextStyle {
            layout: Default::default(),
            font: &font,
            text: "Hello",
            origin: (0.0, 0.0),
            size: f32::NAN,
            color: [0, 0, 0, 255]
        })
        .is_err()
    );
    doc.preview_text(TextStyle {
        layout: Default::default(),
        font: &font,
        text: "Hello",
        origin: (0.0, 0.0),
        size: 20.0,
        color: [0, 0, 0, 255],
    })
    .unwrap();
    doc.cancel();
    assert!(!doc.dirty());
    assert!(!doc.can_undo());
    assert!(doc.pixels().iter().all(|p| *p == [255; 4]));
}

#[test]
fn cancelled_text_render_keeps_the_destination_untouched() {
    let font = font();
    let mut pixels = vec![[17; 4]; 64 * 64];
    assert!(!paint_rust::text::render_cancellable(
        &mut pixels,
        64,
        64,
        None,
        TextStyle {
            layout: Default::default(),
            font: &font,
            text: "Cancelled",
            origin: (0.0, 0.0),
            size: 32.0,
            color: [255; 4]
        },
        &std::sync::atomic::AtomicBool::new(true)
    ));
    assert!(pixels.iter().all(|pixel| *pixel == [17; 4]));
}

#[test]
fn center_and_right_alignment_use_each_lines_measured_advance() {
    use ab_glyph::{Font, ScaleFont};
    use paint_rust::text::{Alignment, TextLayout};
    let font = font();
    let scaled = font.as_scaled(32.0);
    let glyph = scaled.glyph_id('M');
    let advance = 2.0 * scaled.h_advance(glyph) + scaled.kern(glyph, glyph) + 7.0;
    let raster = |alignment, x| {
        let mut pixels = vec![[0; 4]; 320 * 80];
        paint_rust::text::render(
            &mut pixels,
            320,
            80,
            None,
            TextStyle {
                font: &font,
                text: "MM",
                origin: (x, 4.0),
                size: 32.0,
                color: [255; 4],
                layout: TextLayout {
                    alignment,
                    letter_spacing: 7.0,
                    ..Default::default()
                },
            },
        );
        pixels
    };
    assert_eq!(
        raster(Alignment::Center, 160.0),
        raster(Alignment::Left, 160.0 - advance * 0.5)
    );
    assert_eq!(
        raster(Alignment::Right, 160.0),
        raster(Alignment::Left, 160.0 - advance)
    );
}

#[test]
fn line_spacing_moves_later_lines_and_hard_text_has_binary_coverage() {
    use paint_rust::text::TextLayout;
    let font = font();
    let raster = |line_spacing, antialias| {
        let mut pixels = vec![[0; 4]; 160 * 160];
        paint_rust::text::render(
            &mut pixels,
            160,
            160,
            None,
            TextStyle {
                font: &font,
                text: "M\nM",
                origin: (5.0, 5.0),
                size: 32.0,
                color: [255; 4],
                layout: TextLayout {
                    line_spacing,
                    antialias,
                    ..Default::default()
                },
            },
        );
        pixels
    };
    let normal = raster(1.0, true);
    let spaced = raster(2.0, true);
    let last_row = |pixels: &Vec<[u8; 4]>| pixels.iter().rposition(|p| p[3] > 0).unwrap() / 160;
    assert!(last_row(&spaced) > last_row(&normal) + 20);
    assert_eq!(&normal[..160 * 30], &spaced[..160 * 30]);
    assert!(normal.iter().any(|p| p[3] > 0 && p[3] < 255));
    assert!(raster(1.0, false).iter().all(|p| p[3] == 0 || p[3] == 255));
}

#[test]
fn invalid_text_layout_is_rejected_before_modifying_pixels() {
    use paint_rust::text::TextLayout;
    let font = font();
    for layout in [
        TextLayout {
            letter_spacing: f32::NAN,
            ..Default::default()
        },
        TextLayout {
            line_spacing: 0.0,
            ..Default::default()
        },
        TextLayout {
            line_spacing: f32::INFINITY,
            ..Default::default()
        },
    ] {
        let mut pixels = vec![[23, 45, 67, 255]; 64 * 64];
        assert!(!paint_rust::text::render_cancellable(
            &mut pixels,
            64,
            64,
            None,
            TextStyle {
                font: &font,
                text: "Test",
                origin: (0.0, 0.0),
                size: 20.0,
                color: [255; 4],
                layout
            },
            &std::sync::atomic::AtomicBool::new(false)
        ));
        assert!(pixels.iter().all(|p| *p == [23, 45, 67, 255]));
    }
}

#[test]
fn font_loader_accepts_valid_files_and_rejects_invalid_oversized_or_cancelled_reads() {
    use ab_glyph::Font;
    use std::sync::atomic::AtomicBool;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("chosen.ttf");
    let definitions = eframe::egui::FontDefinitions::default();
    let name = &definitions.families[&eframe::egui::FontFamily::Proportional][0];
    std::fs::write(&path, definitions.font_data[name].font.as_ref()).unwrap();
    let loaded = paint_rust::text::load_font_cancellable(&path, &AtomicBool::new(false)).unwrap();
    assert!(
        paint_rust::text::load_font_face_cancellable(&path, u32::MAX, &AtomicBool::new(false))
            .is_err()
    );
    assert_eq!(loaded.glyph_id('M'), font().glyph_id('M'));
    assert!(paint_rust::text::load_font_cancellable(&path, &AtomicBool::new(true)).is_err());
    std::fs::write(&path, b"not a font").unwrap();
    assert!(paint_rust::text::load_font_cancellable(&path, &AtomicBool::new(false)).is_err());
    std::fs::File::create(&path)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    assert!(
        paint_rust::text::load_font_cancellable(&path, &AtomicBool::new(false))
            .unwrap_err()
            .contains("16 MiB")
    );
}

#[test]
fn decorations_cover_spaces_follow_alignment_and_preserve_selection() {
    use paint_rust::text::{self, Alignment, TextLayout};
    let font = font();
    let draw = |alignment, origin, selection, underline, strikethrough, antialias| {
        let mut pixels = vec![[0; 4]; 200 * 100];
        text::render(
            &mut pixels,
            200,
            100,
            selection,
            TextStyle {
                font: &font,
                text: "    \n  ",
                origin,
                size: 32.0,
                color: [80, 120, 220, 255],
                layout: TextLayout {
                    alignment,
                    underline,
                    strikethrough,
                    antialias,
                    ..Default::default()
                },
            },
        );
        pixels
    };
    let underline = draw(Alignment::Left, (20.3, 10.3), None, true, false, true);
    let strike = draw(Alignment::Left, (20.3, 10.3), None, false, true, true);
    let both = draw(Alignment::Left, (20.3, 10.3), None, true, true, true);
    assert!(underline.iter().any(|p| p[3] > 0 && p[3] < 255));
    assert!(strike.iter().any(|p| p[3] > 0));
    assert!(
        underline
            .iter()
            .zip(&strike)
            .all(|(a, b)| a[3] == 0 || b[3] == 0)
    );
    for i in 0..both.len() {
        assert_eq!(
            both[i],
            if underline[i][3] > 0 {
                underline[i]
            } else {
                strike[i]
            }
        );
    }
    let selection = Selection {
        x: 25,
        y: 0,
        width: 10,
        height: 100,
        mask: None,
        ellipse: None,
    };
    let clipped = draw(
        Alignment::Left,
        (20.3, 10.3),
        Some(selection),
        true,
        true,
        true,
    );
    for (i, pixel) in clipped.iter().enumerate() {
        assert_eq!(
            *pixel,
            if (25..35).contains(&(i % 200)) {
                both[i]
            } else {
                [0; 4]
            }
        );
    }
    let hard = draw(Alignment::Left, (20.3, 10.3), None, true, true, false);
    assert!(hard.iter().all(|p| p[3] == 0 || p[3] == 255));
    let right = draw(Alignment::Right, (180.0, 10.3), None, true, true, true);
    let center = draw(Alignment::Center, (100.0, 10.3), None, true, true, true);
    assert!(
        right
            .iter()
            .enumerate()
            .filter(|(_, p)| p[3] > 0)
            .all(|(i, _)| i % 200 < 180)
    );
    assert!(
        center
            .iter()
            .enumerate()
            .filter(|(_, p)| p[3] > 0)
            .any(|(i, _)| i % 200 < 100)
    );
    assert!(
        center
            .iter()
            .enumerate()
            .filter(|(_, p)| p[3] > 0)
            .any(|(i, _)| i % 200 >= 100)
    );
}

#[test]
fn shaping_composes_marks_and_reorders_embedded_rtl_runs() {
    use ab_glyph::Font;
    use paint_rust::text::shaping;
    use std::sync::atomic::AtomicBool;
    let font = font();
    let face = rustybuzz::Face::from_slice(font.font_data(), 0).unwrap();
    let shape =
        |text| shaping::line(&face, &font, text, 32.0, 0.0, &AtomicBool::new(false)).unwrap();
    let composed = shape("é");
    let combining = shape("e\u{301}");
    assert_eq!(composed.glyphs.len(), combining.glyphs.len());
    for (a, b) in composed.glyphs.iter().zip(&combining.glyphs) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.position, b.position);
    }
    let mixed = shape("A\u{202e}BC\u{202c}D");
    let letters: Vec<_> = mixed
        .glyphs
        .iter()
        .map(|g| g.id)
        .filter(|id| "ABCD".chars().any(|c| font.glyph_id(c) == *id))
        .collect();
    assert_eq!(
        letters,
        "ACBD".chars().map(|c| font.glyph_id(c)).collect::<Vec<_>>()
    );
    assert!(shaping::line(&face, &font, "cancelled", 32.0, 0.0, &AtomicBool::new(true)).is_none());
}

#[test]
fn text_decoration_uses_font_position_and_face_index_is_validated() {
    use ab_glyph::{Font, ScaleFont};
    use paint_rust::text::{self, TextLayout};
    let font = font();
    let face = rustybuzz::Face::from_slice(font.font_data(), 0).unwrap();
    let metrics = face.underline_metrics().unwrap();
    let size = 64.0;
    let baseline = 10.0 + font.as_scaled(size).ascent();
    let expected = baseline - metrics.position as f32 * size / font.height_unscaled();
    let mut pixels = vec![[0; 4]; 200 * 100];
    text::render(
        &mut pixels,
        200,
        100,
        None,
        TextStyle {
            font: &font,
            text: "    ",
            origin: (10.0, 10.0),
            size,
            color: [255; 4],
            layout: TextLayout {
                underline: true,
                ..Default::default()
            },
        },
    );
    let mut mass = 0.0;
    let mut moment = 0.0;
    for (i, p) in pixels.iter().enumerate() {
        mass += f64::from(p[3]);
        moment += (i / 200) as f64 * f64::from(p[3]) + 0.5 * f64::from(p[3]);
    }
    assert!(mass > 0.0);
    assert!((moment / mass - f64::from(expected)).abs() < 0.6);
    let before = pixels.clone();
    assert!(!text::render_cancellable(
        &mut pixels,
        200,
        100,
        None,
        TextStyle {
            font: &font,
            text: "invalid face",
            origin: (0.0, 0.0),
            size,
            color: [255; 4],
            layout: TextLayout {
                font_index: u32::MAX,
                ..Default::default()
            }
        },
        &std::sync::atomic::AtomicBool::new(false)
    ));
    assert_eq!(pixels, before);
}

#[test]
fn optional_ligatures_are_disabled_when_letter_spacing_is_requested() {
    use ab_glyph::Font;
    use paint_rust::text::shaping;
    use std::sync::atomic::AtomicBool;
    let font = font();
    let face = rustybuzz::Face::from_slice(font.font_data(), 0).unwrap();
    let joined = shaping::line(&face, &font, "ffi", 32.0, 0.0, &AtomicBool::new(false)).unwrap();
    let spaced = shaping::line(&face, &font, "ffi", 32.0, 4.0, &AtomicBool::new(false)).unwrap();
    assert!(joined.glyphs.len() < 3);
    assert_eq!(spaced.glyphs.len(), 3);
    assert!(spaced.width > joined.width);
}
