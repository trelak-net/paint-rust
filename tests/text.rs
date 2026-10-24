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
            font: &font,
            text: "Hello",
            origin: (0.0, 0.0),
            size: f32::NAN,
            color: [0, 0, 0, 255]
        })
        .is_err()
    );
    doc.preview_text(TextStyle {
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
