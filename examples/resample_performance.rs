//! Repeatable resize throughput and cooperative cancellation measurements.
use paint_rust::resample::{self, Filter, Options};
use std::{
    hint::black_box,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
fn main() {
    let size = std::env::args()
        .nth(1)
        .map_or(2048, |v| v.parse::<u32>().expect("integer size"));
    assert!((64..=4096).contains(&size));
    let pixels: Vec<_> = (0..size * size)
        .map(|i| {
            [
                (i % 251) as u8,
                ((i / size) % 241) as u8,
                (i % 239) as u8,
                ((i % 191) + 64) as u8,
            ]
        })
        .collect();
    println!(
        "{size} square to {} square; 32 MiB filtered-row cache maximum",
        size / 2
    );
    for filter in [
        Filter::Area,
        Filter::Bilinear,
        Filter::Bicubic,
        Filter::BicubicSmooth,
        Filter::Lanczos,
        Filter::Adaptive,
    ] {
        for gamma_correct in [false, true] {
            let start = Instant::now();
            let output = resample::resize(
                &pixels,
                (size, size),
                (size / 2, size / 2),
                Options {
                    filter,
                    gamma_correct,
                },
                &AtomicBool::new(false),
            )
            .unwrap();
            black_box(&output);
            println!(
                "{} gamma={gamma_correct}: {:.2} ms",
                filter.name(),
                start.elapsed().as_secs_f64() * 1000.0
            );
        }
    }
    let cancel = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(20));
            cancel.store(true, Ordering::Relaxed);
        });
        let start = Instant::now();
        let result = resample::resize(
            &pixels,
            (size, size),
            (size * 2, size * 2),
            Options {
                filter: Filter::Lanczos,
                gamma_correct: true,
            },
            &cancel,
        );
        println!(
            "Cancellation requested at 20 ms; returned after {:.2} ms; cancelled={}",
            start.elapsed().as_secs_f64() * 1000.0,
            result.is_err()
        );
    });
}
