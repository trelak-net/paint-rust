use paint_rust::{brush::Options, document::Document};

fn stroke(doc: &mut Document, a: (f32, f32), b: (f32, f32), options: Options, erase: bool) {
    doc.brush_segment(a, b, 7.0, [200, 40, 80, 128], options, erase);
}

#[test]
fn subdividing_and_repeating_a_stroke_does_not_accumulate_opacity() {
    for blend in [false, true] {
        for erase in [false, true] {
            let options = Options {
                hardness: 0.5,
                opacity: 0.6,
                antialias: true,
                blend,
                ..Default::default()
            };
            let mut whole = Document::new(32, 16, [20, 100, 180, 200]).unwrap();
            let mut split = whole.snapshot();
            whole.begin("Brush");
            split.begin("Brush");
            stroke(&mut whole, (3.5, 8.5), (27.5, 8.5), options, erase);
            for _ in 0..3 {
                for x in 3..27 {
                    stroke(
                        &mut split,
                        (x as f32 + 0.5, 8.5),
                        (x as f32 + 1.5, 8.5),
                        options,
                        erase,
                    );
                }
            }
            assert_eq!(whole.pixels(), split.pixels());
            assert!(whole.commit());
            assert!(whole.undo());
            assert!(whole.pixels().iter().all(|p| *p == [20, 100, 180, 200]));
        }
    }
}

#[test]
fn soft_edges_eraser_and_blend_have_explicit_alpha_behavior() {
    let mut doc = Document::new(11, 11, [0, 0, 255, 255]).unwrap();
    doc.begin("Blend");
    stroke(
        &mut doc,
        (5.5, 5.5),
        (5.5, 5.5),
        Options {
            blend: true,
            ..Default::default()
        },
        false,
    );
    assert_eq!(doc.pixels()[60], [100, 20, 167, 255]);
    doc.cancel();
    doc.begin("Erase");
    stroke(
        &mut doc,
        (5.5, 5.5),
        (5.5, 5.5),
        Options {
            hardness: 0.0,
            opacity: 0.5,
            antialias: true,
            blend: false,
            ..Default::default()
        },
        true,
    );
    assert_eq!(doc.pixels()[60], [0, 0, 255, 127]);
    assert!(doc.pixels()[61][3] > 127 && doc.pixels()[61][3] < 255);
    assert_eq!(doc.pixels()[64], [0, 0, 255, 255]);
    doc.cancel();
    assert!(doc.pixels().iter().all(|p| *p == [0, 0, 255, 255]));
}

#[test]
fn coverage_is_reset_between_transactions_and_invalid_input_is_ignored() {
    let mut doc = Document::new(5, 5, [100, 100, 100, 255]).unwrap();
    let options = Options {
        opacity: 0.5,
        ..Default::default()
    };
    for expected in [127, 63] {
        doc.begin("Erase");
        stroke(&mut doc, (2.5, 2.5), (2.5, 2.5), options, true);
        doc.commit();
        assert_eq!(doc.pixels()[12][3], expected);
    }
    let before = doc.pixels().to_vec();
    doc.begin("Invalid");
    stroke(&mut doc, (f32::NAN, 0.0), (0.0, 0.0), options, false);
    assert!(!doc.commit());
    assert_eq!(doc.pixels(), before);
}

#[test]
fn brush_blend_modes_match_layer_compositing_and_do_not_accumulate() {
    use paint_rust::document::{BlendMode, Layer};
    let background = [80, 130, 200, 123];
    let color = [190, 75, 35, 137];
    for mode in BlendMode::ALL
        .into_iter()
        .filter(|mode| *mode != BlendMode::Normal)
    {
        let mut upper = Layer::new("Paint".into(), vec![color]);
        upper.blend_mode = mode;
        upper.opacity = 153;
        let layers = Document::from_layers(
            1,
            1,
            vec![Layer::new("Base".into(), vec![background]), upper],
            1,
        )
        .unwrap();
        let expected = layers.composite()[0];
        let mut document = Document::new(1, 1, background).unwrap();
        document.begin("Blended brush");
        let options = Options {
            blend: true,
            blend_mode: mode,
            opacity: 0.6,
            ..Default::default()
        };
        for _ in 0..3 {
            document.brush_segment((0.5, 0.5), (0.5, 0.5), 1.0, color, options, false);
        }
        assert_eq!(document.pixels()[0], expected, "{mode:?}");
        assert!(document.commit());
        document.undo();
        assert_eq!(document.pixels()[0], background);
        document.redo();
        assert_eq!(document.pixels()[0], expected);
    }
}

#[test]
fn aliased_strokes_ignore_hardness_but_keep_opacity_and_selection() {
    for erase in [false, true] {
        let mut results = Vec::new();
        for hardness in [0.0, 0.5, 1.0] {
            let mut document = Document::new(11, 11, [10, 20, 30, 255]).unwrap();
            document.set_selection(Some(paint_rust::document::Selection {
                x: 2,
                y: 2,
                width: 7,
                height: 7,
                ellipse: None,
                mask: None,
            }));
            document.begin("Stroke");
            stroke(
                &mut document,
                (3.5, 5.5),
                (7.5, 5.5),
                Options {
                    hardness,
                    opacity: 0.5,
                    antialias: false,
                    ..Options::default()
                },
                erase,
            );
            assert_eq!(document.pixels()[5 * 11 + 1], [10, 20, 30, 255]);
            assert_ne!(document.pixels()[5 * 11 + 5], [10, 20, 30, 255]);
            results.push(document.pixels().to_vec());
        }
        assert_eq!(results[0], results[1]);
        assert_eq!(results[1], results[2]);
    }
}
