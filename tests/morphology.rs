use paint_rust::{
    document::Selection,
    effects::{self, Effect, MorphologyOperation as Op},
};
use std::sync::atomic::AtomicBool;
fn run(pixels: &[[u8; 4]], w: u32, h: u32, radius: u32, operation: Op) -> Vec<[u8; 4]> {
    effects::render(
        pixels,
        w,
        h,
        None,
        Effect::Morphology { radius, operation },
        &AtomicBool::new(false),
    )
    .unwrap()
}
#[test]
fn morphology_expands_and_shrinks_binary_objects_and_removes_isolated_defects() {
    let mut image = vec![[0; 4]; 49];
    image[24] = [20, 80, 160, 255];
    let dilated = run(&image, 7, 7, 1, Op::Dilate);
    assert_eq!(dilated.iter().filter(|p| p[3] > 0).count(), 9);
    for y in 2..=4 {
        for x in 2..=4 {
            assert_eq!(dilated[y * 7 + x], [20, 80, 160, 255]);
        }
    }
    assert_eq!(run(&dilated, 7, 7, 1, Op::Erode), image);
    assert_eq!(run(&image, 7, 7, 1, Op::Open), vec![[0; 4]; 49]);
    let mut hole = vec![[20, 80, 160, 255]; 49];
    hole[24] = [255, 0, 0, 0];
    assert_eq!(run(&hole, 7, 7, 1, Op::Close), vec![[20, 80, 160, 255]; 49]);
}
#[test]
fn separable_extrema_match_direct_square_neighborhoods_for_partial_alpha() {
    let image: Vec<_> = (0..30u32)
        .map(|i| {
            [
                (i * 73 % 256) as u8,
                (i * 137 % 256) as u8,
                (i * 31 % 256) as u8,
                (i * 53 % 256) as u8,
            ]
        })
        .collect();
    for radius in [1, 2, 6] {
        for op in [Op::Dilate, Op::Erode] {
            let output = run(&image, 6, 5, radius, op);
            for y in 0..5i32 {
                for x in 0..6i32 {
                    let maximum = op == Op::Dilate;
                    let mut channels = [if maximum { 0u32 } else { 65025 }; 4];
                    for dy in -(radius as i32)..=radius as i32 {
                        for dx in -(radius as i32)..=radius as i32 {
                            let p =
                                image[((y + dy).clamp(0, 4) * 6 + (x + dx).clamp(0, 5)) as usize];
                            let a = u32::from(p[3]);
                            for c in 0..4 {
                                let v = if c == 3 { a * 255 } else { u32::from(p[c]) * a };
                                channels[c] = if maximum {
                                    channels[c].max(v)
                                } else {
                                    channels[c].min(v)
                                };
                            }
                        }
                    }
                    let mut expected = [0; 4];
                    let alpha = channels[3];
                    for c in 0..3 {
                        expected[c] = (channels[c] * 255 + alpha / 2)
                            .checked_div(alpha)
                            .unwrap_or(0) as u8;
                    }
                    expected[3] = ((alpha + 127) / 255) as u8;
                    assert_eq!(output[(y * 6 + x) as usize], expected);
                }
            }
        }
    }
}
#[test]
fn morphology_neutral_selection_cancel_and_bounds() {
    let source = vec![[30, 70, 120, 83]; 25];
    for operation in Op::ALL {
        assert_eq!(run(&[[255, 0, 0, 0]], 1, 1, 0, operation), [[255, 0, 0, 0]]);
        assert_eq!(run(&source, 5, 5, 64, operation), source);
        let effect = Effect::Morphology {
            radius: 1,
            operation,
        };
        let mut dot = vec![[255, 0, 0, 0]; 25];
        dot[12] = [0, 0, 255, 255];
        let output = effects::render(
            &dot,
            5,
            5,
            Some(Selection {
                x: 1,
                y: 1,
                width: 1,
                height: 1,
                mask: None,
                ellipse: None,
            }),
            effect,
            &AtomicBool::new(false),
        )
        .unwrap();
        for i in 0..25 {
            if i != 6 {
                assert_eq!(output[i], dot[i]);
            }
        }
        assert!(effects::render(&source, 5, 5, None, effect, &AtomicBool::new(true)).is_none());
        assert_eq!(
            serde_json::from_slice::<Effect>(&serde_json::to_vec(&effect).unwrap()).unwrap(),
            effect
        );
    }
    let invalid = Effect::Morphology {
        radius: 65,
        operation: Op::Dilate,
    };
    assert!(!invalid.valid_settings());
    assert!(effects::render(&source, 5, 5, None, invalid, &AtomicBool::new(false)).is_none());
}
