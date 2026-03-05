//! Criterion benchmarks for Fixed16_16 arithmetic.
//!
//! Baseline numbers before any SIMD/LUT optimizations.
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use doom_types::fixed::Fixed16_16;

fn bench_fixed_mul(c: &mut Criterion) {
    let a = Fixed16_16::from_int(1234);
    let b = Fixed16_16::from_int(567);
    c.bench_function("Fixed16_16::fixed_mul", |bench| {
        bench.iter(|| black_box(a).fixed_mul(black_box(b)));
    });
}

fn bench_fixed_div(c: &mut Criterion) {
    let a = Fixed16_16::from_int(1234);
    let b = Fixed16_16::from_int(7);
    c.bench_function("Fixed16_16::fixed_div", |bench| {
        bench.iter(|| black_box(a).fixed_div(black_box(b)));
    });
}

fn bench_fixed_mul_chain(c: &mut Criterion) {
    // Simulates a column-renderer inner loop: multiply a series of fixed values.
    let vals: Vec<Fixed16_16> = (1..=64).map(|i| Fixed16_16::from_int(i)).collect();
    c.bench_function("Fixed16_16::fixed_mul x64 chain", |bench| {
        bench.iter(|| {
            vals.iter().fold(Fixed16_16::from_int(1), |acc, &v| {
                acc.fixed_mul(black_box(v))
            })
        });
    });
}

criterion_group!(
    benches,
    bench_fixed_mul,
    bench_fixed_div,
    bench_fixed_mul_chain
);
criterion_main!(benches);
