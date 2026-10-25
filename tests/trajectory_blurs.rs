use paint_rust::{
    document::Selection,
    effects::{self, Effect},
};
use std::sync::atomic::AtomicBool;
fn render(source: &[[u8; 4]], w: u32, h: u32, effect: Effect) -> Vec<[u8; 4]> {
    effects::render(source, w, h, None, effect, &AtomicBool::new(false)).unwrap()
}
fn effects() -> [Effect; 3] {
    [
        Effect::RadialBlur {
            angle: 90.0,
            center: [0.5, 0.5],
        },
        Effect::ZoomBlur {
            amount: 0.8,
            center: [0.5, 0.5],
        },
        Effect::FragmentBlur {
            distance: 1.0,
            fragments: 4,
            angle: 0.0,
        },
    ]
}
#[test]
fn neutral_blurs_preserve_hidden_rgb_and_all_alpha_values() {
    let pixels: Vec<_> = (0..=255).map(|v| [v, 255 - v, 123, v]).collect();
    for effect in [
        Effect::RadialBlur {
            angle: 0.0,
            center: [0.2, 0.8],
        },
        Effect::ZoomBlur {
            amount: 0.0,
            center: [0.0, 1.0],
        },
        Effect::FragmentBlur {
            distance: 0.0,
            fragments: 32,
            angle: 15.0,
        },
    ] {
        assert_eq!(render(&pixels, 16, 16, effect), pixels);
    }
}
#[test]
fn constant_color_is_preserved_and_hidden_colors_do_not_bleed() {
    for effect in effects() {
        let constant = vec![[40, 90, 170, 83]; 25];
        assert_eq!(render(&constant, 5, 5, effect), constant);
        let mut pixels = vec![[255, 0, 0, 0]; 25];
        pixels[12] = [0, 0, 255, 255];
        let output = render(&pixels, 5, 5, effect);
        assert!(output.iter().all(|p| p[0] == 0 && p[1] == 0));
        assert!(output.iter().any(|p| p[3] > 0));
        assert!(output.iter().filter(|p| p[3] > 0).all(|p| p[2] == 255));
        assert_ne!(output, pixels);
    }
}
#[test]
fn fragment_offsets_have_expected_geometry() {
    let mut pixels = vec![[0; 4]; 25];
    pixels[12] = [0, 0, 255, 255];
    let output = render(&pixels, 5, 5, effects()[2]);
    for i in [7, 11, 13, 17] {
        assert_eq!(output[i], [0, 0, 255, 64]);
    }
    assert_eq!(output[12][3], 0);
}
#[test]
fn selection_cancel_and_invalid_centers_are_handled() {
    let mut pixels = vec![[0; 4]; 25];
    pixels[12] = [20, 80, 150, 255];
    for effect in effects() {
        let selection = Selection {
            x: 1,
            y: 1,
            width: 1,
            height: 1,
            mask: None,
            ellipse: None,
        };
        let output = effects::render(
            &pixels,
            5,
            5,
            Some(selection),
            effect,
            &AtomicBool::new(false),
        )
        .unwrap();
        for i in 0..25 {
            if i != 6 {
                assert_eq!(output[i], pixels[i]);
            }
        }
        assert!(effects::render(&pixels, 5, 5, None, effect, &AtomicBool::new(true)).is_none());
        assert_eq!(
            render(&[[50, 60, 70, 80]], 1, 1, effect),
            [[50, 60, 70, 80]]
        );
    }
    for effect in [
        Effect::ZoomBlur {
            amount: 1.0,
            center: [f32::NAN, 0.0],
        },
        Effect::RadialBlur {
            angle: 181.0,
            center: [0.5, 0.5],
        },
        Effect::FragmentBlur {
            distance: 1.0,
            fragments: 0,
            angle: 0.0,
        },
    ] {
        assert!(!effect.valid_settings());
        assert!(effects::render(&pixels, 5, 5, None, effect, &AtomicBool::new(false)).is_none());
    }
}
