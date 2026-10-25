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
