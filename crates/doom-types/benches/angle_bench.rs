#![allow(missing_docs)]

//! Criterion benchmarks for Bam (Binary Angle Measure) operations.
//!
//! `Bam::sin` / `Bam::cos` are table lookups keyed by `bam >> 19`.
//! These benchmarks establish the lookup throughput baseline.
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use doom_types::Bam;

fn bench_bam_sin(c: &mut Criterion) {
    // SAFETY: init_trig_tables must be called before sin/cos.

    let angle = Bam(0x4000_0000); // 90 degrees
    c.bench_function("Bam::sin", |bencher| {
        bencher.iter(|| black_box(angle).sin())
    });
}

fn bench_bam_cos(c: &mut Criterion) {
    // init_trig_tables is idempotent (AtomicBool guard), safe to call again.

    let angle = Bam(0x4000_0000); // 90 degrees
    c.bench_function("Bam::cos", |bencher| {
        bencher.iter(|| black_box(angle).cos())
    });
}

fn bench_bam_add(c: &mut Criterion) {
    let a = Bam(0x1234_5678);
    let b = Bam(0x8765_4321);
    c.bench_function("Bam wrapping_add", |bencher| {
        bencher.iter(|| black_box(a).wrapping_add(black_box(b)))
    });
}

fn bench_bam_fine_angle(c: &mut Criterion) {
    // Benchmark the `bam >> 19` index computation used inside sin/cos.
    let angle = Bam(0xDEAD_BEEF);
    c.bench_function("Bam::fine_angle", |bencher| {
        bencher.iter(|| black_box(angle).fine_angle())
    });
}

fn bench_bam_sin_loop(c: &mut Criterion) {
    // Simulate a renderer sweeping angles across a horizontal FOV.

    let step = Bam(0x0040_0000); // roughly 0.3 degrees per step
    c.bench_function("Bam::sin sweep x320", |bencher| {
        bencher.iter(|| {
            let mut angle = Bam::ZERO;
            let mut acc = doom_types::Fixed16_16::ZERO;
            for _ in 0..320 {
                acc += black_box(angle).sin();
                angle = angle.wrapping_add(step);
            }
            black_box(acc)
        })
    });
}

criterion_group!(
    angle_benches,
    bench_bam_sin,
    bench_bam_cos,
    bench_bam_add,
    bench_bam_fine_angle,
    bench_bam_sin_loop
);
criterion_main!(angle_benches);
