use paint_rust::{
    document::Document,
    resample::{self, Filter, Options},
};
use std::sync::atomic::AtomicBool;
fn options(filter: Filter, gamma_correct: bool) -> Options {
    Options {
        filter,
        gamma_correct,
    }
}
#[test]
fn gamma_correct_area_filter_preserves_light_and_premultiplies_alpha() {
    let cancel = AtomicBool::new(false);
    for (gamma, expected) in [(false, 128), (true, 188)] {
        let result = resample::resize(
            &[[0, 0, 0, 255], [255; 4]],
            (2, 1),
            (1, 1),
            options(Filter::Area, gamma),
            &cancel,
        )
        .unwrap();
        assert_eq!(result, [[expected, expected, expected, 255]]);
        let result = resample::resize(
            &[[255, 0, 0, 255], [0, 0, 255, 0]],
            (2, 1),
            (1, 1),
            options(Filter::Area, gamma),
            &cancel,
        )
        .unwrap();
        assert_eq!(result, [[255, 0, 0, 128]]);
    }
}
#[test]
fn every_filter_preserves_constants_and_identity_hidden_rgb() {
    let cancel = AtomicBool::new(false);
    for filter in Filter::ALL {
        for gamma in [false, true] {
            for new in [(2, 3), (13, 2), (3, 11), (8, 9)] {
                let result = resample::resize(
                    &[[57, 103, 211, 123]; 35],
                    (5, 7),
                    new,
                    options(filter, gamma),
                    &cancel,
                )
                .unwrap();
                assert!(
                    result.iter().all(|p| *p == [57, 103, 211, 123]),
                    "{filter:?}: {:?}",
                    result[0]
                );
            }
            let source = [[23, 45, 67, 0], [1, 2, 3, 1]];
            assert_eq!(
                resample::resize(&source, (2, 1), (2, 1), options(filter, gamma), &cancel).unwrap(),
                source
            );
        }
    }
}
#[test]
fn severe_aspect_ratio_change_and_transpose_have_consistent_results() {
    let cancel = AtomicBool::new(false);
    let source: Vec<_> = (0..4096).map(|i| [(i % 256) as u8; 4]).collect();
    let horizontal = resample::resize(
        &source,
        (4096, 1),
        (1, 4096),
        options(Filter::Area, false),
        &cancel,
    )
    .unwrap();
    let vertical = resample::resize(
        &source,
        (1, 4096),
        (4096, 1),
        options(Filter::Area, false),
        &cancel,
    )
    .unwrap();
    assert_eq!(horizontal, vertical);
    assert!(horizontal.iter().all(|p| *p == horizontal[0]));
    assert_eq!(horizontal[0][3], 128);
}
#[test]
fn resize_all_layers_restores_selection_and_pixels_with_one_undo() {
    let cancel = AtomicBool::new(false);
    let mut doc = Document::new(4, 3, [30, 50, 70, 128]).unwrap();
    doc.add_layer(true);
    let selection = Some(paint_rust::document::Selection::elliptical(0, 0, 3, 3));
    doc.set_selection(selection.clone());
    let before = doc.layers().to_vec();
    let count = doc.history().count();
    doc.resize_image_with_options_cancellable(7, 8, options(Filter::Lanczos, true), &cancel)
        .unwrap();
    assert_eq!((doc.width, doc.height), (7, 8));
    assert_eq!(doc.history().count(), count + 1);
    assert_eq!(doc.layers().len(), 2);
    assert!(
        doc.layers()[0]
            .pixels()
            .iter()
            .all(|p| *p == [30, 50, 70, 128])
    );
    doc.undo();
    assert!(doc.layers() == before);
    assert_eq!((doc.width, doc.height), (4, 3));
    assert_eq!(doc.selection(), selection);
    assert!(
        doc.resize_image_with_options_cancellable(8, 8, Options::default(), &AtomicBool::new(true))
            .is_err()
    );
    assert!(doc.layers() == before);
}
#[test]
fn invalid_dimensions_and_buffers_are_rejected_before_sampling() {
    let cancel = AtomicBool::new(false);
    assert!(resample::resize(&[], (0, 1), (2, 2), Options::default(), &cancel).is_err());
    assert!(resample::resize(&[[0; 4]], (2, 2), (2, 2), Options::default(), &cancel).is_err());
    assert!(
        resample::resize(
            &[[0; 4]],
            (1, 1),
            (u32::MAX, 1),
            Options::default(),
            &cancel
        )
        .is_err()
    );
}
