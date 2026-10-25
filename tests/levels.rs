use paint_rust::{
    document::Selection,
    effects::{self, Effect},
    levels::{Channel, Levels},
};
use std::sync::atomic::AtomicBool;

#[test]
fn default_levels_are_identity_and_gamma_and_endpoints_are_explicit() {
    let source: Vec<_> = (0..=255).map(|i| [i, 255 - i, i, i]).collect();
    assert_eq!(
        effects::render(
            &source,
            256,
            1,
            None,
            Effect::Levels(Levels::default()),
            &AtomicBool::new(false)
        )
        .unwrap(),
        source
    );
    let mut levels = Levels::default();
    levels.channels[0] = Channel {
        input_black: 16,
        input_white: 235,
        ..Default::default()
    };
    let source = [[0, 16, 235, 17], [255, 16, 235, 0]];
    assert_eq!(
        effects::render(
            &source,
            2,
            1,
            None,
            Effect::Levels(levels),
            &AtomicBool::new(false)
        )
        .unwrap(),
        [[0, 0, 255, 17], [255, 0, 255, 0]]
    );
    levels.channels[0] = Channel {
        gamma: 2.0,
        ..Default::default()
    };
    assert_eq!(
        effects::render(
            &[[64, 64, 64, 99]],
            1,
            1,
            None,
            Effect::Levels(levels),
            &AtomicBool::new(false)
        )
        .unwrap(),
        [[128, 128, 128, 99]]
    );
    levels.channels[0] = Channel {
        output_black: 200,
        output_white: 50,
        ..Default::default()
    };
    assert_eq!(
        effects::render(
            &[[0, 255, 0, 99]],
            1,
            1,
            None,
            Effect::Levels(levels),
            &AtomicBool::new(false)
        )
        .unwrap(),
        [[200, 50, 200, 99]]
    );
}

#[test]
fn channel_levels_follow_master_and_respect_selection() {
    let mut levels = Levels::default();
    levels.channels[0] = Channel {
        output_black: 10,
        output_white: 245,
        ..Default::default()
    };
    levels.channels[1] = Channel {
        output_black: 255,
        output_white: 0,
        ..Default::default()
    };
    let selection = Selection {
        x: 1,
        y: 0,
        width: 1,
        height: 1,
        mask: None,
        ellipse: None,
    };
    assert_eq!(
        effects::render(
            &[[0, 0, 0, 255]; 2],
            2,
            1,
            Some(selection),
            Effect::Levels(levels),
            &AtomicBool::new(false)
        )
        .unwrap(),
        [[0, 0, 0, 255], [245, 10, 10, 255]]
    );
}

#[test]
fn auto_levels_ignores_transparent_outliers_and_keeps_constant_channels() {
    let source = [[0, 0, 0, 0], [50, 100, 50, 128], [150, 100, 150, 255]];
    assert_eq!(
        effects::render(
            &source,
            3,
            1,
            None,
            Effect::AutoLevels(0.0),
            &AtomicBool::new(false)
        )
        .unwrap(),
        [[0, 0, 0, 0], [0, 100, 0, 128], [255, 100, 255, 255]]
    );
    let source = [[0, 100, 0, 255], [50, 100, 50, 255], [150, 100, 150, 255]];
    let selection = Selection {
        x: 1,
        y: 0,
        width: 2,
        height: 1,
        mask: None,
        ellipse: None,
    };
    assert_eq!(
        effects::render(
            &source,
            3,
            1,
            Some(selection),
            Effect::AutoLevels(0.0),
            &AtomicBool::new(false)
        )
        .unwrap(),
        [[0, 100, 0, 255], [0, 100, 0, 255], [255, 100, 255, 255]]
    );
    let transparent = [[37, 59, 71, 0]; 3];
    assert_eq!(
        effects::render(
            &transparent,
            3,
            1,
            None,
            Effect::AutoLevels(0.5),
            &AtomicBool::new(false)
        )
        .unwrap(),
        transparent
    );
}

#[test]
fn invalid_levels_and_cancellation_are_rejected() {
    for channel in [
        Channel {
            input_black: 255,
            ..Default::default()
        },
        Channel {
            gamma: 0.0,
            ..Default::default()
        },
        Channel {
            gamma: f32::NAN,
            ..Default::default()
        },
    ] {
        assert!(
            effects::render(
                &[[128; 4]],
                1,
                1,
                None,
                Effect::Levels(Levels {
                    channels: [channel; 4]
                }),
                &AtomicBool::new(false)
            )
            .is_none()
        );
    }
    for clip in [f32::NAN, -1.0, 6.0] {
        assert!(
            effects::render(
                &[[128; 4]],
                1,
                1,
                None,
                Effect::AutoLevels(clip),
                &AtomicBool::new(false)
            )
            .is_none()
        );
    }
    assert!(
        effects::render(
            &[[128; 4]],
            1,
            1,
            None,
            Effect::AutoLevels(0.5),
            &AtomicBool::new(true)
        )
        .is_none()
    );
}

#[test]
fn auto_levels_clips_the_requested_population_at_both_ends() {
    let mut source = vec![[0, 0, 0, 255]];
    source.extend((0..18).map(|i| {
        let value = 50 + i * 10;
        [value, value, value, 255]
    }));
    source.push([255; 4]);
    let clipped = effects::render(
        &source,
        20,
        1,
        None,
        Effect::AutoLevels(5.0),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(clipped[1], [0, 0, 0, 255]);
    assert_eq!(clipped[18], [255; 4]);
    let full = effects::render(
        &source,
        20,
        1,
        None,
        Effect::AutoLevels(0.0),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(full, source);
}

#[test]
fn histogram_counts_selection_once_and_excludes_hidden_pixels() {
    let source = [
        [10, 20, 30, 255],
        [10, 40, 30, 1],
        [200, 200, 200, 0],
        [99, 99, 99, 255],
    ];
    let selection = Selection {
        x: 0,
        y: 0,
        width: 3,
        height: 1,
        mask: None,
        ellipse: None,
    };
    let bins = paint_rust::levels::histogram(&source, 4, Some(&selection), &AtomicBool::new(false))
        .unwrap();
    assert_eq!(bins[0][10], 2);
    assert_eq!(bins[1][20], 1);
    assert_eq!(bins[1][40], 1);
    assert_eq!(bins[2][30], 2);
    for channel in bins {
        assert_eq!(channel.iter().sum::<u32>(), 2);
    }
    assert!(paint_rust::levels::histogram(&source, 4, None, &AtomicBool::new(true)).is_none());
    assert!(paint_rust::levels::histogram(&source, 0, None, &AtomicBool::new(false)).is_none());
    assert!(paint_rust::levels::histogram(&source, 3, None, &AtomicBool::new(false)).is_none());
}
