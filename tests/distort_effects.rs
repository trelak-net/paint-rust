use paint_rust::{
    document::Selection,
    effects::{self, Effect},
};
use std::sync::atomic::AtomicBool;
fn render(source: &[[u8; 4]], width: u32, effect: Effect) -> Vec<[u8; 4]> {
    effects::render(
        source,
        width,
        source.len() as u32 / width,
        None,
        effect,
        &AtomicBool::new(false),
    )
    .unwrap()
}
#[test]
fn neutral_warps_are_exact_and_constant_fields_stay_constant() {
    let source: Vec<_> = (0..25).map(|i| [i * 5, i * 3, i * 2, i * 7]).collect();
    for effect in [
        Effect::Bulge(0.0),
        Effect::Twist(0.0),
        Effect::FrostedGlass { radius: 0, seed: 1 },
        Effect::Crystalize { size: 1, seed: 1 },
        Effect::Vignette {
            amount: 0.0,
            roundness: 0.5,
        },
    ] {
        assert_eq!(render(&source, 5, effect), source);
    }
    let constant = vec![[20, 100, 180, 128]; 49];
    for effect in [
        Effect::Bulge(-80.0),
        Effect::Bulge(80.0),
        Effect::Twist(360.0),
        Effect::FrostedGlass {
            radius: 10,
            seed: 1,
        },
        Effect::Crystalize { size: 4, seed: 1 },
    ] {
        assert_eq!(render(&constant, 7, effect), constant);
        assert!(effects::render(&constant, 7, 7, None, effect, &AtomicBool::new(true)).is_none());
    }
}
#[test]
fn seeded_distortion_is_repeatable_and_obeys_selection() {
    let source: Vec<_> = (0..256)
        .map(|i| [i as u8, (i % 16 * 16) as u8, (i / 16 * 16) as u8, 255])
        .collect();
    let selection = Selection {
        x: 4,
        y: 4,
        width: 8,
        height: 8,
        mask: None,
        ellipse: None,
    };
    for effect in [
        Effect::Bulge(60.0),
        Effect::Twist(180.0),
        Effect::FrostedGlass {
            radius: 4,
            seed: 42,
        },
        Effect::Crystalize { size: 5, seed: 42 },
    ] {
        let first = render(&source, 16, effect);
        assert_ne!(first, source);
        assert_eq!(first, render(&source, 16, effect));
        let masked = effects::render(
            &source,
            16,
            16,
            Some(selection.clone()),
            effect,
            &AtomicBool::new(false),
        )
        .unwrap();
        for (i, p) in masked.iter().enumerate() {
            if !(4..12).contains(&(i % 16)) || !(4..12).contains(&(i / 16)) {
                assert_eq!(*p, source[i]);
            }
        }
    }
    for effect in [
        Effect::Bulge(f32::NAN),
        Effect::Twist(f32::INFINITY),
        Effect::RedEye(f32::NAN),
        Effect::Vignette {
            amount: 1.0,
            roundness: f32::NAN,
        },
    ] {
        assert!(effects::render(&source, 16, 16, None, effect, &AtomicBool::new(false)).is_none());
    }
}
#[test]
fn photo_corrections_target_red_and_edges_without_changing_alpha() {
    let source = [[220, 40, 30, 128], [40, 200, 30, 200], [220, 40, 30, 0]];
    assert_eq!(
        render(&source, 3, Effect::RedEye(1.5)),
        [[35, 40, 30, 128], source[1], source[2]]
    );
    let source = vec![[200, 180, 160, 128]; 81];
    let output = render(
        &source,
        9,
        Effect::Vignette {
            amount: 1.0,
            roundness: 0.5,
        },
    );
    assert_eq!(output[40], source[40]);
    assert!(output[0][0] < source[0][0]);
    assert!(output.iter().all(|p| p[3] == 128));
}
