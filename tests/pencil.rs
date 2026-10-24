use paint_rust::document::{Document, Selection};

const INK: [u8; 4] = [20, 80, 210, 127];

#[test]
fn pencil_connects_samples_in_every_direction_with_one_pixel_per_major_step() {
    for end in [
        (14.0, 10.0),
        (10.0, 14.0),
        (2.0, 10.0),
        (6.0, 2.0),
        (14.0, 8.0),
        (8.0, 14.0),
        (8.0, 8.0),
    ] {
        let mut doc = Document::new(16, 16, [255; 4]).unwrap();
        doc.begin("Pencil");
        doc.pencil((8.0, 8.0), end, INK);
        doc.commit();
        let expected = (end.0 - 8.0).abs().max((end.1 - 8.0).abs()) as usize + 1;
        assert_eq!(doc.pixels().iter().filter(|p| **p == INK).count(), expected);
        assert_eq!(doc.sample(8, 8), Some(INK));
        assert_eq!(doc.sample(end.0 as i32, end.1 as i32), Some(INK));
        let forward = doc.pixels().to_vec();
        doc.undo();
        assert!(doc.pixels().iter().all(|p| *p == [255; 4]));
        doc.begin("Reverse pencil");
        doc.pencil(end, (8.0, 8.0), INK);
        doc.commit();
        assert_eq!(doc.pixels(), forward);
    }
}

#[test]
fn pencil_clips_off_canvas_and_respects_selection_and_cancel() {
    let mut doc = Document::new(16, 16, [255; 4]).unwrap();
    doc.set_selection(Some(Selection::elliptical(4, 4, 8, 8)));
    doc.begin("Pencil");
    doc.pencil((-1e9, 8.0), (1e9, 8.0), INK);
    for x in 0..16 {
        let inside = (4..12).contains(&x);
        assert_eq!(doc.sample(x, 8), Some(if inside { INK } else { [255; 4] }));
    }
    doc.cancel();
    assert!(doc.pixels().iter().all(|p| *p == [255; 4]));
    assert!(!doc.can_undo());
}
