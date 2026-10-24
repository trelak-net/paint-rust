use paint_rust::{
    affine::{self, FreeTransform},
    document::{Document, Selection},
};
use std::sync::atomic::AtomicBool;

fn render(
    source: &[[u8; 4]],
    width: u32,
    height: u32,
    selection: Option<&Selection>,
    parameters: FreeTransform,
) -> Vec<[u8; 4]> {
    affine::render(
        source,
        width,
        height,
        selection,
        parameters,
        &AtomicBool::new(false),
    )
    .unwrap()
}

#[test]
fn identity_preserves_hidden_rgb_and_invalid_or_cancelled_work_has_no_output() {
    let source = [[12, 34, 56, 0], [200, 100, 50, 123]];
    assert_eq!(
        render(&source, 2, 1, None, FreeTransform::default()),
        source
    );
    assert!(
        affine::render(
            &source,
            2,
            1,
            None,
            FreeTransform::default(),
            &AtomicBool::new(true)
        )
        .is_none()
    );
    for p in [
        FreeTransform {
            scale_x: 0.0,
            ..Default::default()
        },
        FreeTransform {
            angle: f32::NAN,
            ..Default::default()
        },
    ] {
        assert!(affine::render(&source, 2, 1, None, p, &AtomicBool::new(false)).is_none());
    }
}

#[test]
fn quarter_turn_and_mirror_match_exact_pixel_orientation() {
    let source = [
        [10, 0, 0, 255],
        [20, 0, 0, 255],
        [30, 0, 0, 255],
        [40, 0, 0, 255],
    ];
    let turned = render(
        &source,
        2,
        2,
        None,
        FreeTransform {
            angle: 90.0,
            smooth: false,
            ..Default::default()
        },
    );
    assert_eq!(turned, [source[2], source[0], source[3], source[1]]);
    let mirrored = render(
        &source,
        2,
        2,
        None,
        FreeTransform {
            flip_x: true,
            smooth: false,
            ..Default::default()
        },
    );
    assert_eq!(mirrored, [source[1], source[0], source[3], source[2]]);
}

#[test]
fn selected_translation_uses_frozen_pixels_and_undo_restores_mask() {
    let mut doc = Document::from_pixels(
        5,
        1,
        vec![
            [10, 0, 0, 255],
            [20, 0, 0, 255],
            [30, 0, 0, 255],
            [40, 0, 0, 255],
            [50, 0, 0, 255],
        ],
    )
    .unwrap();
    let selection = Selection {
        x: 0,
        y: 0,
        width: 3,
        height: 1,
        ellipse: None,
        mask: None,
    };
    doc.set_selection(Some(selection.clone()));
    let source = doc.pixels().to_vec();
    let output = render(
        &source,
        5,
        1,
        Some(&selection),
        FreeTransform {
            offset_x: 2.0,
            smooth: false,
            ..Default::default()
        },
    );
    assert_eq!(output, [[0; 4], [0; 4], source[0], source[1], source[2]]);
    doc.begin("Free transform");
    doc.publish_preview(output).unwrap();
    doc.commit();
    doc.set_selection(None);
    doc.undo();
    assert_eq!(doc.pixels(), source);
    assert_eq!(doc.selection(), Some(selection));
    assert!(!doc.dirty());
}

#[test]
fn smooth_scaling_respects_mask_and_does_not_bleed_invisible_color() {
    let mut source = vec![[0, 0, 255, 0]; 9];
    source[4] = [255, 0, 0, 255];
    let selection = Selection::elliptical(1, 1, 1, 1);
    let output = render(
        &source,
        3,
        3,
        Some(&selection),
        FreeTransform {
            scale_x: 2.0,
            scale_y: 2.0,
            ..Default::default()
        },
    );
    assert_eq!(output[0], [255, 0, 0, 64]);
    assert_eq!(output[4], [255, 0, 0, 255]);
    assert!(output.iter().all(|pixel| pixel[..3] == [255, 0, 0]));
}

#[test]
fn copying_keeps_original_pixels_and_off_canvas_output_is_clipped() {
    let source = [[200, 0, 0, 255], [0; 4], [0; 4]];
    let selection = Selection::elliptical(0, 0, 1, 1);
    let copied = render(
        &source,
        3,
        1,
        Some(&selection),
        FreeTransform {
            offset_x: 1.0,
            copy: true,
            smooth: false,
            ..Default::default()
        },
    );
    assert_eq!(copied, [source[0], source[0], [0; 4]]);
    let moved = render(
        &source,
        3,
        1,
        Some(&selection),
        FreeTransform {
            offset_x: 100.0,
            smooth: false,
            ..Default::default()
        },
    );
    assert_eq!(moved, [[0; 4]; 3]);
}
