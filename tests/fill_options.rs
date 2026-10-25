use paint_rust::{
    document::{Document, Layer, Selection},
    selection::{ColorMatch, CombineMode, combine},
};
use std::sync::atomic::AtomicBool;

const RED: [u8; 4] = [255, 0, 0, 255];
fn rect(x: u32, width: u32) -> Selection {
    Selection {
        x,
        y: 0,
        width,
        height: 1,
        mask: None,
        ellipse: None,
    }
}

#[test]
fn fill_respects_selection_connectivity_and_global_matching() {
    for contiguous in [true, false] {
        let mut doc = Document::new(5, 1, [255; 4]).unwrap();
        let selection = combine(5, 1, Some(&rect(0, 2)), &rect(3, 2), CombineMode::Add).unwrap();
        doc.set_selection(Some(selection.clone()));
        doc.begin("Fill");
        assert!(
            doc.fill_sampled_cancellable(
                (0, 0),
                RED,
                ColorMatch {
                    tolerance: 0,
                    contiguous
                },
                false,
                &AtomicBool::new(false)
            )
            .unwrap()
        );
        assert!(doc.commit());
        assert_eq!(
            doc.pixels(),
            if contiguous {
                &[RED, RED, [255; 4], [255; 4], [255; 4]]
            } else {
                &[RED, RED, [255; 4], RED, RED]
            }
        );
        doc.undo();
        assert_eq!(doc.pixels(), &[[255; 4]; 5]);
        assert_eq!(doc.selection(), Some(selection));
        doc.begin("Outside selection");
        assert!(
            !doc.fill_sampled_cancellable(
                (2, 0),
                RED,
                ColorMatch {
                    tolerance: 255,
                    contiguous
                },
                false,
                &AtomicBool::new(false)
            )
            .unwrap()
        );
        assert!(!doc.commit());
        assert!(!doc.dirty());
    }
}

#[test]
fn merged_sampling_changes_only_active_pixels_and_tolerance_is_fixed_to_seed() {
    let lower = Layer::new(
        "Lower".into(),
        vec![
            [100, 100, 100, 255],
            [105, 100, 100, 255],
            [110, 100, 100, 255],
        ],
    );
    let upper = Layer::new("Upper".into(), vec![[0; 4]; 3]);
    let mut doc = Document::from_layers(3, 1, vec![lower.clone(), upper], 1).unwrap();
    doc.begin("Fill");
    doc.fill_sampled_cancellable(
        (0, 0),
        RED,
        ColorMatch {
            tolerance: 5,
            contiguous: true,
        },
        true,
        &AtomicBool::new(false),
    )
    .unwrap();
    doc.commit();
    assert_eq!(doc.pixels(), &[RED, RED, [0; 4]]);
    assert!(doc.layers()[0] == lower);
    assert_eq!(doc.history().count(), 1);
    doc.undo();
    doc.begin("Fill raw");
    doc.fill_sampled_cancellable(
        (0, 0),
        RED,
        ColorMatch {
            tolerance: 0,
            contiguous: true,
        },
        false,
        &AtomicBool::new(false),
    )
    .unwrap();
    doc.commit();
    assert_eq!(doc.pixels(), &[RED; 3]);
}

#[test]
fn cancelled_fill_and_invalid_seed_do_not_publish_changes() {
    let mut doc = Document::new(2, 2, [255; 4]).unwrap();
    let options = ColorMatch {
        tolerance: 255,
        contiguous: false,
    };
    doc.begin("Fill");
    assert!(
        doc.fill_sampled_cancellable((0, 0), RED, options, true, &AtomicBool::new(true))
            .is_err()
    );
    assert_eq!(doc.pixels(), &[[255; 4]; 4]);
    doc.cancel();
    doc.begin("Fill");
    assert!(
        !doc.fill_sampled_cancellable((-1, 0), RED, options, false, &AtomicBool::new(false))
            .unwrap()
    );
    assert!(!doc.commit());
    assert!(!doc.can_undo());
}
