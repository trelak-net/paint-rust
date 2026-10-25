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
fn recolor_matches_tolerance_preserves_alpha_and_obeys_selection() {
    let original = vec![
        [100, 50, 20, 128],
        [105, 50, 20, 255],
        [106, 50, 20, 255],
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
        from: [100, 50, 20, 255],
        to: [0, 150, 220, 255],
        tolerance: 5,
    };
    doc.paint_segment((0.5, 0.5), (4.5, 0.5), 1.0, paint, Options::default());
    assert_eq!(
        doc.pixels(),
        [
            [0, 150, 220, 128],
            [0, 150, 220, 255],
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
