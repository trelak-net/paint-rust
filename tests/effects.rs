use paint_rust::{
    document::{Document, Selection},
    effects::{self, Effect},
};
use std::sync::atomic::AtomicBool;

#[test]
fn identity_adjustments_preserve_every_channel_and_cancellation_stops_work() {
    let source = vec![[20, 130, 240, 100], [50, 60, 70, 0]];
    for effect in [
        Effect::BrightnessContrast {
            brightness: 0,
            contrast: 0,
        },
        Effect::Saturation(1.0),
        Effect::BoxBlur(0),
    ] {
        assert_eq!(
            effects::render(&source, 2, 1, None, effect, &AtomicBool::new(false)).unwrap(),
            source
        );
    }
    assert!(
        effects::render(
            &source,
            2,
            1,
            None,
            Effect::BoxBlur(32),
            &AtomicBool::new(true)
        )
        .is_none()
    );
}

#[test]
fn adjustments_respect_selection_alpha_and_transaction_cancellation() {
    let mut doc = Document::new(2, 2, [200, 100, 50, 123]).unwrap();
    let selection = Selection {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    };
    doc.set_selection(Some(selection));
    let output = effects::render(
        doc.pixels(),
        2,
        2,
        Some(selection),
        Effect::Grayscale,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(output[0][0], output[0][1]);
    assert_eq!(output[0][1], output[0][2]);
    assert_eq!(output[0][3], 123);
    assert_eq!(output[1], [200, 100, 50, 123]);
    doc.begin("Grayscale");
    doc.publish_preview(output).unwrap();
    assert!(doc.dirty());
    doc.cancel();
    assert!(!doc.dirty());
    assert!(!doc.can_undo());
}

#[test]
fn blur_uses_premultiplied_alpha_and_clamps_image_edges() {
    let source = vec![[255, 0, 0, 255], [0, 0, 255, 0], [0, 255, 0, 0]];
    let output = effects::render(
        &source,
        3,
        1,
        None,
        Effect::BoxBlur(1),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(output[0], [255, 0, 0, 170]);
    assert_eq!(output[1], [255, 0, 0, 85]);
    assert_eq!(output[2], [0; 4]);
}
