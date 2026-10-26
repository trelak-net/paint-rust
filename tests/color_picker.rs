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
    assert_eq!(document.sample_area(0, 0, 16, false), None);
    assert!(!document.dirty());
    assert_eq!(document.history().count(), 0);
}

#[test]
fn active_layer_and_merged_sampling_have_distinct_semantics() {
    let mut document = Document::new(2, 2, [255, 0, 0, 255]).unwrap();
    assert!(document.add_layer(false));
    assert_eq!(document.sample_area(0, 0, 15, false), Some([0; 4]));
    assert_eq!(document.sample_area(0, 0, 15, true), Some([255, 0, 0, 255]));
    for y in 0..2 {
        for x in 0..2 {
            assert_eq!(document.sample_area(x, y, 0, true), document.sample(x, y));
        }
    }
}
