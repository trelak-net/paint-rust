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
        Effect::Pixelate(1),
        Effect::Sharpen(0.0),
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
        mask: None,
        ellipse: None,
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    };
    doc.set_selection(Some(selection.clone()));
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

#[test]
fn pixelation_averages_premultiplied_colors_and_partial_edge_blocks() {
    let source = [[255, 0, 0, 255], [0, 255, 0, 0], [0, 0, 255, 100]];
    let output = effects::render(
        &source,
        3,
        1,
        None,
        Effect::Pixelate(2),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(
        output,
        [[255, 0, 0, 128], [255, 0, 0, 128], [0, 0, 255, 100]]
    );
    let selection = Selection {
        mask: None,
        ellipse: None,
        x: 1,
        y: 0,
        width: 1,
        height: 1,
    };
    let selected = effects::render(
        &source,
        3,
        1,
        Some(selection),
        Effect::Pixelate(2),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(selected, [source[0], output[1], source[2]]);
}

#[test]
fn posterization_quantizes_channels_without_changing_alpha_or_unselected_pixels() {
    let source = [[40, 120, 240, 123], [67, 80, 90, 0]];
    let selection = Selection {
        mask: None,
        ellipse: None,
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    };
    let output = effects::render(
        &source,
        2,
        1,
        Some(selection),
        Effect::Posterize(4),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(output, [[0, 85, 255, 123], source[1]]);
}

#[test]
fn malformed_effect_buffers_and_cancelled_new_effects_produce_no_output() {
    let cancel = AtomicBool::new(false);
    assert!(effects::render(&[], 0, 0, None, Effect::Pixelate(8), &cancel).is_none());
    assert!(effects::render(&[[0; 4]], 2, 1, None, Effect::Posterize(4), &cancel).is_none());
    for effect in [Effect::Pixelate(8), Effect::Posterize(4)] {
        assert!(effects::render(&[[0; 4]], 1, 1, None, effect, &AtomicBool::new(true)).is_none());
    }
}

#[test]
fn sharpening_increases_local_contrast_without_changing_alpha() {
    let source = [[80, 80, 80, 255], [120, 120, 120, 255], [80, 80, 80, 255]];
    let output = effects::render(
        &source,
        3,
        1,
        None,
        Effect::Sharpen(1.0),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(
        output,
        [[67, 67, 67, 255], [147, 147, 147, 255], [67, 67, 67, 255]]
    );
}

#[test]
fn sharpening_preserves_unselected_pixels_and_transparent_color_values() {
    let source = [[0, 0, 255, 0], [128, 80, 50, 123], [255, 0, 255, 0]];
    let selection = Selection::elliptical(1, 0, 1, 1);
    let output = effects::render(
        &source,
        3,
        1,
        Some(selection),
        Effect::Sharpen(3.0),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(output, source);
    let output = effects::render(
        &source,
        3,
        1,
        None,
        Effect::Sharpen(3.0),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(output, source);
}
