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
