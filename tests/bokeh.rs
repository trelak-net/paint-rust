use paint_rust::{
    document::Selection,
    effects::{self, Effect},
};
use std::sync::atomic::AtomicBool;
fn effect(radius: f32, gamma: f32, quality: u8) -> Effect {
    Effect::BokehBlur {
        radius,
        gamma,
        quality,
    }
}
fn run(source: &[[u8; 4]], width: u32, height: u32, effect: Effect) -> Vec<[u8; 4]> {
    effects::render(source, width, height, None, effect, &AtomicBool::new(false)).unwrap()
}
#[test]
fn bokeh_constant_images_and_neutral_settings_preserve_bytes() {
    for gamma in [0.1, 1.0, 5.0] {
        for quality in 1..=5 {
            assert_eq!(
                run(&[[40, 90, 170, 83]; 25], 5, 5, effect(10.0, gamma, quality)),
                [[40, 90, 170, 83]; 25]
            );
        }
    }
    let source = [[255, 0, 0, 0], [0, 0, 255, 37]];
    assert_eq!(run(&source, 2, 1, effect(0.0, 2.0, 5)), source);
    assert_eq!(
        run(&[[17, 29, 35, 41]], 1, 1, effect(100.0, 0.1, 5)),
        [[17, 29, 35, 41]]
    );
}
#[test]
fn bokeh_spreads_a_disk_symmetrically_without_hidden_color_bleeding() {
    let mut source = vec![[255, 0, 0, 0]; 81];
    source[40] = [0, 0, 255, 255];
    let output = run(&source, 9, 9, effect(2.0, 2.0, 3));
    assert!(output[40][3] > 0 && output[40][3] < 255);
    assert!(output[39][3] > 0 && output[31][3] > 0);
    for i in 0..81 {
        assert_eq!(output[i], output[80 - i]);
        if output[i][3] > 0 {
            assert_eq!(&output[i][..3], &[0, 0, 255]);
        }
    }
    assert_eq!(output[0], [0; 4]);
}
#[test]
fn gamma_boost_increases_brightness_without_changing_alpha() {
    let source = [[0, 0, 0, 255], [255, 255, 255, 255], [0, 0, 0, 255]];
    let low = run(&source, 3, 1, effect(2.0, 0.5, 3));
    let high = run(&source, 3, 1, effect(2.0, 3.0, 3));
    for i in 0..3 {
        assert!(high[i][0] > low[i][0]);
        assert_eq!(high[i][3], low[i][3]);
    }
}
#[test]
fn bokeh_selection_cancel_and_invalid_parameters_are_handled() {
    let source = [[0, 0, 0, 255], [255, 255, 255, 255], [0, 0, 0, 255]];
    let settings = effect(2.0, 1.0, 2);
    let full = run(&source, 3, 1, settings);
    let selected = effects::render(
        &source,
        3,
        1,
        Some(Selection {
            x: 1,
            y: 0,
            width: 1,
            height: 1,
            mask: None,
            ellipse: None,
        }),
        settings,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(selected, [source[0], full[1], source[2]]);
    assert!(effects::render(&source, 3, 1, None, settings, &AtomicBool::new(true)).is_none());
    assert_eq!(
        serde_json::from_slice::<Effect>(&serde_json::to_vec(&settings).unwrap()).unwrap(),
        settings
    );
    for settings in [
        effect(f32::NAN, 1.0, 2),
        effect(2.0, 0.0, 2),
        effect(2.0, 1.0, 6),
    ] {
        assert!(!settings.valid_settings());
        assert!(effects::render(&source, 3, 1, None, settings, &AtomicBool::new(false)).is_none());
    }
}
