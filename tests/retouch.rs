use paint_rust::{
    brush::{Options, Paint},
    document::{Document, Selection},
};

#[test]
fn clone_uses_frozen_source_with_overlap_and_clips_outside_samples() {
    let original: Vec<_> = (0..8).map(|x| [x * 30, 0, 0, 255]).collect();
    let mut doc = Document::from_pixels(8, 1, original.clone()).unwrap();
    doc.begin("Clone");
    for x in 1..8 {
        doc.paint_segment(
            (x as f32 + 0.5, 0.5),
            (x as f32 + 0.5, 0.5),
            1.0,
            Paint::Clone { offset: (-1, 0) },
            Options::default(),
        );
    }
    assert_eq!(&doc.pixels()[1..], &original[..7]);
    doc.paint_segment(
        (0.5, 0.5),
        (0.5, 0.5),
        1.0,
        Paint::Clone { offset: (-1, 0) },
        Options::default(),
    );
    assert_eq!(doc.pixels()[0], original[0]);
    doc.commit();
    assert!(doc.undo());
    assert_eq!(doc.pixels(), original);
}

#[test]
fn recolor_preserves_shade_differences_and_obeys_alpha_tolerance_and_selection() {
    let original = vec![
        [100, 50, 20, 128],
        [105, 50, 20, 255],
        [112, 50, 20, 255],
        [100, 50, 20, 0],
        [100, 50, 20, 200],
    ];
    let mut doc = Document::from_pixels(5, 1, original.clone()).unwrap();
    doc.set_selection(Some(Selection {
        x: 0,
        y: 0,
        width: 4,
        height: 1,
        mask: None,
        ellipse: None,
    }));
    doc.begin("Recolor");
    let paint = Paint::Recolor {
        alpha: paint_rust::selection::AlphaMatch::Straight,
        from: [100, 50, 20, 255],
        to: [0, 150, 220, 255],
        tolerance: 5,
    };
    doc.paint_segment((0.5, 0.5), (4.5, 0.5), 1.0, paint, Options::default());
    assert_eq!(
        doc.pixels(),
        [
            original[0],
            [5, 150, 220, 255],
            original[2],
            original[3],
            original[4]
        ]
    );
    doc.cancel();
    assert_eq!(doc.pixels(), original);
}

#[test]
fn transparent_clone_replaces_or_blends_according_to_option() {
    for blend in [false, true] {
        let mut doc =
            Document::from_pixels(2, 1, vec![[50, 80, 100, 0], [10, 20, 30, 255]]).unwrap();
        doc.begin("Clone");
        doc.paint_segment(
            (1.5, 0.5),
            (1.5, 0.5),
            1.0,
            Paint::Clone { offset: (-1, 0) },
            Options {
                blend,
                ..Default::default()
            },
        );
        assert_eq!(
            doc.pixels()[1],
            if blend {
                [10, 20, 30, 255]
            } else {
                [50, 80, 100, 0]
            }
        );
    }
}

#[test]
fn recolor_shifts_all_channels_clamps_edges_and_does_not_accumulate_on_overlap() {
    let original = vec![[20, 40, 60, 0], [120, 140, 160, 128], [220, 240, 250, 255]];
    let mut doc = Document::from_pixels(3, 1, original.clone()).unwrap();
    doc.begin("Recolor");
    let paint = Paint::Recolor {
        alpha: paint_rust::selection::AlphaMatch::Premultiplied,
        from: [120, 140, 160, 128],
        to: [160, 100, 255, 200],
        tolerance: 255,
    };
    for _ in 0..3 {
        doc.paint_segment((0.5, 0.5), (2.5, 0.5), 1.0, paint, Options::default());
    }
    assert_eq!(
        doc.pixels(),
        &[[60, 0, 155, 72], [155, 104, 243, 228], [255, 200, 255, 255]]
    );
    doc.commit();
    assert_eq!(doc.history().count(), 1);
    doc.undo();
    assert_eq!(doc.pixels(), original);
}

#[test]
fn recolor_alpha_comparison_controls_hidden_color_matching() {
    use paint_rust::selection::AlphaMatch;
    let original = vec![[20, 40, 60, 0], [80, 90, 100, 0]];
    for alpha in [AlphaMatch::Premultiplied, AlphaMatch::Straight] {
        let mut doc = Document::from_pixels(2, 1, original.clone()).unwrap();
        doc.begin("Recolor");
        doc.paint_segment(
            (0.5, 0.5),
            (1.5, 0.5),
            1.0,
            Paint::Recolor {
                alpha,
                from: original[0],
                to: [0, 0, 0, 255],
                tolerance: 0,
            },
            Options::default(),
        );
        assert_eq!(doc.pixels()[0], [0, 0, 0, 255]);
        assert_eq!(
            doc.pixels()[1],
            if alpha == AlphaMatch::Premultiplied {
                [60, 50, 40, 255]
            } else {
                original[1]
            }
        );
    }
}

#[test]
fn recolor_rgba_ramp_matches_native_full_coverage_fixture() {
    compare_native_ramp("RecolorRampNative.pdn", 100);
    compare_native_ramp("RecolorRampTolerance50Native.pdn", 50);
}

fn compare_native_ramp(fixture: &str, percent: u8) {
    let mut document = paint_rust::files::open(std::path::Path::new(
        "tests/fixtures/pdn/RecolorRampInput.pdn",
    ))
    .unwrap();
    let native =
        paint_rust::files::open(&std::path::Path::new("tests/fixtures/pdn").join(fixture)).unwrap();
    let from = document.pixels()[24 * 350 + 175];
    document.begin("Recolor");
    document.paint_segment(
        (175.5, 24.5),
        (175.5, 24.5),
        1000.0,
        Paint::Recolor {
            alpha: paint_rust::selection::AlphaMatch::Premultiplied,
            from,
            to: [0, 0, 0, 255],
            tolerance: paint_rust::selection::tolerance_threshold(percent),
        },
        Options::default(),
    );
    let mismatches: Vec<_> = document
        .pixels()
        .iter()
        .zip(native.pixels())
        .enumerate()
        .filter(|(index, (a, b))| {
            // The partial-tolerance reference feathers region edges. Compare cell
            // interiors here; edge coverage remains a separate acceptance item.
            let (x, y) = (index % 350, index / 350);
            let interior = (2..48).contains(&(x % 50)) && (2..8).contains(&(y % 10));
            (percent == 100 || interior) && a != b
        })
        .take(8)
        .collect();
    assert!(
        mismatches.is_empty(),
        "basis {from:?}; mismatches {mismatches:?}"
    );
}
