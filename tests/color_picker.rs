use paint_rust::document::Document;

#[test]
fn area_sample_weights_alpha_clips_edges_and_preserves_single_pixels() {
    let document = Document::from_pixels(
        3,
        1,
        vec![[255, 0, 0, 255], [0, 255, 0, 0], [0, 0, 255, 128]],
    )
    .unwrap();
    assert_eq!(document.sample_area(1, 0, 0, false), Some([0, 255, 0, 0]));
    assert_eq!(document.sample_area(0, 0, 1, false), Some([255, 0, 0, 128]));
    assert_eq!(
        document.sample_area(1, 0, 1, false),
        Some([170, 0, 85, 128])
    );
    assert_eq!(document.sample_area(-1, 0, 1, false), None);
    assert_eq!(document.sample_area(3, 0, 1, false), None);
    assert_eq!(document.sample_area(0, 0, 26, false), None);
    assert!(!document.dirty());
    assert_eq!(document.history().count(), 0);
}

#[test]
fn active_layer_and_merged_sampling_have_distinct_semantics() {
    let mut document = Document::new(2, 2, [255, 0, 0, 255]).unwrap();
    assert!(document.add_layer(false));
    assert_eq!(document.sample_area(0, 0, 25, false), Some([0; 4]));
    assert_eq!(document.sample_area(0, 0, 25, true), Some([255, 0, 0, 255]));
    for y in 0..2 {
        for x in 0..2 {
            assert_eq!(document.sample_area(x, y, 0, true), document.sample(x, y));
        }
    }
}

#[test]
fn largest_sample_includes_the_full_square_and_excludes_outer_pixels() {
    let mut pixels = vec![[0, 0, 255, 255]; 53 * 53];
    for y in 1..52 {
        for x in 1..52 {
            pixels[y * 53 + x] = [255, 0, 0, 255];
        }
    }
    let document = Document::from_pixels(53, 53, pixels).unwrap();
    for merged in [false, true] {
        assert_eq!(
            document.sample_area(26, 26, 25, merged),
            Some([255, 0, 0, 255])
        );
        assert_eq!(
            document.sample_area(25, 26, 25, merged),
            Some([250, 0, 5, 255])
        );
    }
}

#[test]
fn sample_averaging_matches_native_interior_and_clipped_edge_fixture() {
    let image = image::load_from_memory(include_bytes!("fixtures/color-picker/alpha-stripes.png"))
        .unwrap()
        .to_rgba8();
    let document = Document::from_pixels(
        image.width(),
        image.height(),
        image.pixels().map(|p| p.0).collect(),
    )
    .unwrap();
    assert_eq!(
        document.sample_area(49, 49, 1, false),
        Some([170, 0, 85, 128])
    );
    assert_eq!(
        document.sample_area(0, 49, 1, false),
        Some([255, 0, 0, 128])
    );
}
