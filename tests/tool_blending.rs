use paint_rust::{
    document::{BlendMode, Document, GradientStyle, Shape, ShapeStyle},
    gradient::{self, Gradient, GradientMode, GradientShape},
    shapes,
    text::{self, TextLayout, TextStyle},
};
use std::sync::atomic::AtomicBool;
fn expected(dst: [u8; 4], src: [u8; 4], mode: BlendMode, opacity: u8) -> [u8; 4] {
    let mut doc = Document::new(1, 1, dst).unwrap();
    doc.import_layer(1, 1, &[src], "Top").unwrap();
    doc.set_layer_style("Top".into(), opacity, mode);
    doc.composite()[0]
}

#[test]
fn solid_shapes_lines_and_gradients_match_layer_compositing_in_every_mode() {
    let cancel = AtomicBool::new(false);
    let dst = [80, 140, 210, 130];
    let src = [190, 70, 110, 170];
    for mode in BlendMode::ALL {
        let expected = expected(dst, src, mode, 255);
        for shape in [Shape::Rectangle, Shape::Line, Shape::Ellipse] {
            let pixels = shapes::render(
                &[dst; 16],
                4,
                4,
                None,
                shapes::Parameters {
                    shape,
                    from: (0.0, 0.0),
                    to: (4.0, 4.0),
                    style: ShapeStyle {
                        paint_mode: Some(mode),
                        color: src,
                        width: 4.0,
                        filled: true,
                        antialias: false,
                        outline: None,
                    },
                },
                &cancel,
            )
            .unwrap();
            assert_eq!(pixels[5], expected, "{mode:?} {shape:?}");
        }
        for operation in [
            GradientMode::Blend,
            GradientMode::Replace,
            GradientMode::Transparency,
        ] {
            let pixels = gradient::render(
                &[dst; 16],
                4,
                4,
                None,
                Gradient {
                    from: (0.0, 0.0),
                    to: (4.0, 0.0),
                    style: GradientStyle {
                        blend_mode: mode,
                        start: src,
                        end: src,
                        shape: GradientShape::Linear,
                        mode: operation,
                    },
                },
                &cancel,
            )
            .unwrap();
            match operation {
                GradientMode::Blend => assert_eq!(pixels[5], expected),
                GradientMode::Replace => assert_eq!(pixels[5], src),
                GradientMode::Transparency => assert_eq!(&pixels[5][..3], &dst[..3]),
            }
        }
    }
}

#[test]
fn text_blend_modes_apply_glyph_coverage_and_preserve_unpainted_pixels() {
    let definitions = eframe::egui::FontDefinitions::default();
    let name = &definitions.families[&eframe::egui::FontFamily::Proportional][0];
    let font = ab_glyph::FontArc::try_from_vec(definitions.font_data[name].font.to_vec()).unwrap();
    let dst = [80, 140, 210, 130];
    let src = [190, 70, 110, 170];
    let draw = |pixels: &mut [_], color, mode| {
        text::render(
            pixels,
            64,
            64,
            None,
            TextStyle {
                font: &font,
                text: "M",
                origin: (4.0, 4.0),
                size: 48.0,
                color,
                layout: TextLayout {
                    blend_mode: mode,
                    ..Default::default()
                },
            },
        )
    };
    let mut mask = vec![[0; 4]; 64 * 64];
    draw(&mut mask, [255; 4], BlendMode::Normal);
    assert!(mask.iter().any(|p| p[3] > 0 && p[3] < 255));
    for mode in BlendMode::ALL {
        let mut pixels = vec![dst; 64 * 64];
        draw(&mut pixels, src, mode);
        for (pixel, coverage) in pixels.iter().zip(&mask) {
            assert_eq!(
                *pixel,
                if coverage[3] == 0 {
                    dst
                } else {
                    expected(dst, src, mode, coverage[3])
                },
                "{mode:?}"
            );
        }
    }
}
