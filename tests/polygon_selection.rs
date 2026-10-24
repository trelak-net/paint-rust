use paint_rust::{document::Document, selection};

fn selected_count(points: &[(f64, f64)]) -> usize {
    let mut doc = Document::new(6, 6, [255; 4]).unwrap();
    doc.set_selection(Some(selection::polygon(6, 6, points).unwrap()));
    doc.clear_pixels();
    doc.pixels().iter().filter(|p| **p == [0; 4]).count()
}

#[test]
fn polygon_rectangle_and_concave_shape_have_expected_coverage() {
    let rectangle = [(1.0, 1.0), (5.0, 1.0), (5.0, 5.0), (1.0, 5.0)];
    assert_eq!(selected_count(&rectangle), 16);
    let concave = [
        (0.0, 0.0),
        (4.0, 0.0),
        (4.0, 2.0),
        (2.0, 2.0),
        (2.0, 4.0),
        (0.0, 4.0),
    ];
    assert_eq!(selected_count(&concave), 12);
    let reversed: Vec<_> = concave.iter().copied().rev().collect();
    assert_eq!(selected_count(&reversed), 12);
}

#[test]
fn polygon_clips_to_canvas_and_degenerate_geometry_remains_empty() {
    assert_eq!(
        selected_count(&[(-10.0, -10.0), (20.0, -10.0), (20.0, 20.0), (-10.0, 20.0)]),
        36
    );
    assert_eq!(selected_count(&[(1.0, 1.0), (2.0, 2.0), (3.0, 3.0)]), 0);
    assert_eq!(
        selected_count(&[(10.0, 10.0), (20.0, 10.0), (20.0, 20.0)]),
        0
    );
}

#[test]
fn polygon_validates_points_and_even_odd_rule_preserves_a_hole() {
    assert!(selection::polygon(6, 6, &[(0.0, 0.0), (1.0, 1.0)]).is_err());
    assert!(selection::polygon(6, 6, &[(0.0, 0.0), (1.0, 1.0), (f64::NAN, 2.0)]).is_err());
    let ring = [
        (0.0, 0.0),
        (6.0, 0.0),
        (6.0, 6.0),
        (0.0, 6.0),
        (0.0, 0.0),
        (2.0, 2.0),
        (4.0, 2.0),
        (4.0, 4.0),
        (2.0, 4.0),
        (2.0, 2.0),
    ];
    assert_eq!(selected_count(&ring), 32);
}
