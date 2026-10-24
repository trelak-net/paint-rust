//! Repeatable CPU timings; run with cargo run --release --example performance.
use paint_rust::document::{BlendMode, Document, Layer};
use std::{hint::black_box, time::Instant};

fn measure<T>(name: &str, work: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let result = black_box(work());
    println!("{name}: {:.2} ms", start.elapsed().as_secs_f64() * 1000.0);
    result
}

fn main() {
    let size = std::env::args()
        .nth(1)
        .map_or(2048, |arg| arg.parse::<u32>().expect("integer size"));
    assert!((1..=4096).contains(&size));
    let mut layers = Vec::new();
    for index in 0..4 {
        let mut layer = Layer::new(
            format!("Layer {index}"),
            vec![[80, 120, 160, 200]; (size * size) as usize],
        );
        if index > 0 {
            layer.blend_mode = BlendMode::Multiply;
        }
        layers.push(layer);
    }
    let mut document = Document::from_layers(size, size, layers, 3).unwrap();
    println!("{size} x {size}, four layers, three Multiply blends");
    drop(measure("Full composite", || document.composite()));
    document.begin("Stroke");
    let snapshot = document.snapshot();
    measure("Brush frame with a renderer snapshot", || {
        document.stroke((10.0, 10.0), (30.0, 30.0), 12.0, [0, 0, 0, 255], false)
    });
    drop(snapshot);
    measure("Next brush frame without a new snapshot", || {
        document.stroke((30.0, 30.0), (50.0, 50.0), 12.0, [0, 0, 0, 255], false)
    });
    measure("Commit stroke", || document.commit());
    measure("Invert active layer", || document.invert());
    measure("Flatten layers", || document.flatten());
    println!(
        "Retained history: {:.1} MiB RAM, {:.1} MiB disk, {} undo steps",
        document.history_bytes() as f64 / 1048576.0,
        document.history_disk_bytes() as f64 / 1048576.0,
        document.history().count(),
    );
    assert!(measure("Undo flatten", || document.undo()));
    assert_eq!(document.layers().len(), 4);
    assert!(measure("Redo flatten", || document.redo()));
    assert_eq!(document.layers().len(), 1);
    assert!(document.history_error().is_none());
}
