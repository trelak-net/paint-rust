use paint_rust::selection::color_cancellable;
use std::sync::atomic::AtomicBool;

fn selected(selection: &paint_rust::selection::Selection, x: u32, y: u32) -> bool {
    let mask = selection.mask.as_ref().unwrap();
    let mx = i64::from(x) - mask.x;
    let my = i64::from(y) - mask.y;
    mx >= 0
        && my >= 0
        && mx < i64::from(mask.width)
        && my < i64::from(mask.height)
        && mask.pixels[my as usize * mask.width as usize + mx as usize] != 0
}

#[test]
fn connected_color_selection_does_not_cross_diagonals_or_row_boundaries() {
    let pixels = [
        [0, 0, 0, 255],
        [255; 4],
        [255; 4],
        [255; 4],
        [0, 0, 0, 255],
        [255; 4],
        [0, 0, 0, 255],
        [255; 4],
        [0, 0, 0, 255],
    ];
    let cancel = AtomicBool::new(false);
    let connected = color_cancellable(3, 3, &pixels, (0, 0), 0, true, &cancel).unwrap();
    let global = color_cancellable(3, 3, &pixels, (0, 0), 0, false, &cancel).unwrap();
    for y in 0..3 {
        for x in 0..3 {
            assert_eq!(selected(&connected, x, y), (x, y) == (0, 0));
            assert_eq!(
                selected(&global, x, y),
                pixels[(y * 3 + x) as usize][0] == 0
            );
        }
    }
    let pixels = [[0; 4], [0; 4], [255; 4], [255; 4], [0; 4], [0; 4]];
    let wrap = color_cancellable(3, 2, &pixels, (2, 0), 0, true, &cancel).unwrap();
    assert!(!selected(&wrap, 0, 1));
}

#[test]
fn tolerance_is_inclusive_and_transparent_hidden_color_is_ignored() {
    let cancel = AtomicBool::new(false);
    let pixels = [[20, 40, 60, 255], [30, 40, 60, 255], [32, 40, 60, 255]];
    let selection = color_cancellable(3, 1, &pixels, (0, 0), 5, true, &cancel).unwrap();
    assert!(selected(&selection, 1, 0));
    assert!(!selected(&selection, 2, 0));
    let pixels = [[255, 0, 80, 0], [0, 255, 0, 0], [0, 0, 0, 1], [255; 4]];
    let exact = color_cancellable(4, 1, &pixels, (0, 0), 0, true, &cancel).unwrap();
    assert!(selected(&exact, 1, 0));
    assert!(!selected(&exact, 2, 0));
    let all = color_cancellable(4, 1, &pixels, (0, 0), 255, true, &cancel).unwrap();
    assert_eq!((all.width, all.height), (4, 1));
    assert!(all.mask.unwrap().pixels.iter().all(|p| *p == 1));
}

#[test]
fn invalid_images_seeds_and_cancellation_are_rejected() {
    let cancel = AtomicBool::new(false);
    assert!(color_cancellable(0, 1, &[], (0, 0), 0, true, &cancel).is_err());
    assert!(color_cancellable(2, 2, &[[0; 4]], (0, 0), 0, true, &cancel).is_err());
    assert!(color_cancellable(1, 1, &[[0; 4]], (1, 0), 0, true, &cancel).is_err());
    assert_eq!(
        color_cancellable(1, 1, &[[0; 4]], (0, 0), 0, true, &AtomicBool::new(true)).unwrap_err(),
        paint_rust::files::CANCELLED
    );
}

#[test]
fn color_distance_uses_all_channels_and_preserves_exact_zero() {
    use paint_rust::selection::{AlphaMatch, tolerance_threshold};
    let straight = AlphaMatch::Straight;
    assert_eq!(straight.distance([0; 4], [255; 4]), 255);
    assert_eq!(straight.distance([0, 0, 0, 255], [20, 0, 0, 255]), 10);
    assert!(straight.distance([0, 0, 0, 255], [20, 20, 20, 255]) > 10);
    assert_eq!(straight.distance([1, 0, 0, 0], [0; 4]), 1);
    assert_eq!(
        AlphaMatch::Premultiplied.distance([255, 0, 0, 0], [0, 255, 0, 0]),
        0
    );
    assert_eq!(
        [0, 25, 50, 75, 100].map(tolerance_threshold),
        [0, 16, 64, 143, 255]
    );
    for percent in 0..100 {
        assert!(tolerance_threshold(percent) <= tolerance_threshold(percent + 1));
    }
}
