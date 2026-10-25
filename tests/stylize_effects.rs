use paint_rust::effects::{self, Effect};
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
fn noise_is_reproducible_keeps_alpha_and_supports_neutral_settings() {
    let source = vec![[128, 128, 128, 180]; 256];
    let effect = Effect::AddNoise {
        amount: 20,
        coverage: 100,
        monochrome: true,
        seed: 42,
    };
    let first = render(&source, 16, effect);
    assert_eq!(first, render(&source, 16, effect));
    assert_ne!(first, source);
    assert!(
        first
            .iter()
            .all(|p| p[0] == p[1] && p[1] == p[2] && p[3] == 180)
    );
    assert_ne!(
        first,
        render(
            &source,
            16,
            Effect::AddNoise {
                amount: 20,
                coverage: 100,
                monochrome: true,
                seed: 43
            }
        )
    );
    for (amount, coverage) in [(0, 100), (100, 0)] {
        assert_eq!(
            render(
                &source,
                16,
                Effect::AddNoise {
                    amount,
                    coverage,
                    monochrome: false,
                    seed: 1
                }
            ),
            source
        );
    }
    let hidden = [[200, 50, 10, 0]];
    assert_eq!(render(&hidden, 1, effect), hidden);
}
#[test]
fn median_removes_impulse_and_supports_percentile_endpoints() {
    let mut source = vec![[10, 20, 30, 255]; 25];
    source[12] = [255, 255, 255, 255];
    assert_eq!(
        render(
            &source,
            5,
            Effect::Median {
                radius: 1,
                percentile: 50
            }
        )[12],
        [10, 20, 30, 255]
    );
    assert_eq!(
        render(
            &source,
            5,
            Effect::Median {
                radius: 1,
                percentile: 100
            }
        )[12],
        [255; 4]
    );
    source[12] = [255, 0, 0, 0];
    let output = render(
        &source,
        5,
        Effect::Median {
            radius: 1,
            percentile: 50,
        },
    );
    assert_eq!(output[12], source[12]);
    assert_eq!(output[11], [10, 20, 30, 255]);
    assert_eq!(
        render(
            &source,
            5,
            Effect::Median {
                radius: 0,
                percentile: 50
            }
        ),
        source
    );
}
#[test]
fn edges_and_emboss_have_neutral_flat_fields_and_keep_alpha() {
    let source = vec![[10, 50, 100, 120]; 25];
    assert!(
        render(&source, 5, Effect::EdgeDetect)
            .iter()
            .all(|p| *p == [0, 0, 0, 120])
    );
    assert!(
        render(&source, 5, Effect::Emboss(45.0))
            .iter()
            .all(|p| *p == [128, 128, 128, 120])
    );
    let mut edge = vec![[0, 0, 0, 255]; 25];
    for y in 0..5 {
        for x in 3..5 {
            edge[y * 5 + x] = [255; 4];
        }
    }
    assert!(render(&edge, 5, Effect::EdgeDetect)[12][0] > 0);
    let left = render(&edge, 5, Effect::Emboss(0.0));
    let right = render(&edge, 5, Effect::Emboss(180.0));
    assert!(left[12][0] > 128 && right[12][0] < 128);
    for effect in [
        Effect::Median {
            radius: 10,
            percentile: 50,
        },
        Effect::EdgeDetect,
        Effect::Emboss(40.0),
    ] {
        assert!(effects::render(&source, 5, 5, None, effect, &AtomicBool::new(true)).is_none());
    }
    assert!(
        effects::render(
            &source,
            5,
            5,
            None,
            Effect::Emboss(f32::NAN),
            &AtomicBool::new(false)
        )
        .is_none()
    );
}
