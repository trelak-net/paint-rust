use paint_rust::{
    document::{Document, Selection},
    selection::{self, CombineMode, clipboard},
};
use std::sync::atomic::AtomicBool;
fn mask(selection: Selection) -> Vec<bool> {
    let mut doc = Document::new(12, 10, [0, 0, 0, 255]).unwrap();
    doc.set_selection(Some(selection));
    doc.invert();
    doc.pixels().iter().map(|p| p[0] == 255).collect()
}
fn decode(text: &str) -> Selection {
    clipboard::decode(12, 10, text, &AtomicBool::new(false)).unwrap()
}
#[test]
fn geometry_roundtrips_rectangles_ellipses_holes_and_irregular_masks_exactly() {
    let rectangle = Selection {
        x: 2,
        y: 2,
        width: 8,
        height: 6,
        mask: None,
        ellipse: None,
    };
    let ellipse = Selection::elliptical(2, 2, 8, 6);
    let polygon = selection::polygon(12, 10, &[(1.0, 1.0), (10.0, 2.0), (4.0, 8.0)]).unwrap();
    let hole = selection::combine_cancellable(
        12,
        10,
        Some(&rectangle),
        &Selection::elliptical(4, 3, 4, 4),
        CombineMode::Subtract,
        &AtomicBool::new(false),
    )
    .unwrap();
    for source in [rectangle, ellipse, polygon, hole] {
        let text = clipboard::encode(12, 10, Some(&source), &AtomicBool::new(false)).unwrap();
        assert!(text.contains("polygonList"));
        assert_eq!(mask(decode(&text)), mask(source));
    }
}
#[test]
fn polygon_json_clips_and_uses_even_odd_holes_with_either_winding() {
    let result = mask(decode(
        r#"{"polygonList":["-2,-2,20,-2,20,20,-2,20","3,3,3,7,7,7,7,3"]}"#,
    ));
    for y in 0..10 {
        for x in 0..12 {
            assert_eq!(
                result[y * 12 + x],
                !((3..7).contains(&x) && (3..7).contains(&y))
            );
        }
    }
    let reverse = decode(r#"{"polygonList":["0,0,12,0,12,10,0,10","3,3,7,3,7,7,3,7"]}"#);
    assert_eq!(mask(reverse), result);
}
#[test]
fn full_canvas_and_empty_geometry_have_compact_explicit_representations() {
    let text = clipboard::encode(12, 10, None, &AtomicBool::new(false)).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["polygonList"].as_array().unwrap().len(), 1);
    assert!(mask(decode(&text)).iter().all(|v| *v));
    let empty = decode(r#"{"polygonList":[]}"#);
    assert!(empty.mask.is_some());
    assert_eq!((empty.width, empty.height), (0, 0));
    assert!(mask(empty.clone()).iter().all(|v| !*v));
    assert_eq!(
        clipboard::encode(12, 10, Some(&empty), &AtomicBool::new(false)).unwrap(),
        r#"{"polygonList":[]}"#
    );
}
#[test]
fn malformed_large_or_cancelled_geometry_is_rejected() {
    for text in [
        "not json",
        r#"{"polygonList":["0,0,1"]}"#,
        r#"{"polygonList":["0,0,1,0,NaN,1"]}"#,
        r#"{"polygonList":["0,0,1,0,1000000001,1"]}"#,
        r#"{"polygonList":[false]}"#,
    ] {
        assert!(clipboard::decode(12, 10, text, &AtomicBool::new(false)).is_err());
    }
    assert!(
        clipboard::decode(
            12,
            10,
            &" ".repeat(8 * 1024 * 1024 + 1),
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(clipboard::decode(12, 10, r#"{"polygonList":[]}"#, &AtomicBool::new(true)).is_err());
    assert!(clipboard::encode(12, 10, None, &AtomicBool::new(true)).is_err());
}

#[test]
fn external_clipboard_roundtrip_and_native_inversion_match_pixel_masks() {
    let cancel = AtomicBool::new(false);
    let original = clipboard::decode(
        17,
        13,
        include_str!("fixtures/selection/MaskInput.json"),
        &cancel,
    )
    .unwrap();
    let copied = clipboard::decode(
        17,
        13,
        include_str!("fixtures/selection/MaskNativeCopied.json"),
        &cancel,
    )
    .unwrap();
    let inverted = clipboard::decode(
        17,
        13,
        include_str!("fixtures/selection/MaskNativeInverted.json"),
        &cancel,
    )
    .unwrap();
    let raster = |selection| {
        let mut doc = Document::new(17, 13, [0, 0, 0, 255]).unwrap();
        doc.set_selection(Some(selection));
        doc.invert();
        doc.pixels().iter().map(|p| p[0] == 255).collect::<Vec<_>>()
    };
    let expected = raster(original);
    assert_eq!(raster(copied), expected);
    assert_eq!(
        raster(inverted),
        expected.into_iter().map(|value| !value).collect::<Vec<_>>()
    );
}
