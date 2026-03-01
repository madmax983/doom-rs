//! Criterion benchmarks for Fixed16_16 arithmetic — `FixedMul` throughput baseline.
//!
//! These numbers document pre-optimization performance so that future SIMD
//! or LUT changes can be compared against this baseline.
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use doom_types::Fixed16_16;

fn bench_fixed_mul(c: &mut Criterion) {
    let a = Fixed16_16::from_int(100);
    let b = Fixed16_16::from_int(37);
    c.bench_function("Fixed16_16::fixed_mul", |bencher| {
        bencher.iter(|| black_box(a).fixed_mul(black_box(b)))
    });
}

fn bench_fixed_div(c: &mut Criterion) {
    let a = Fixed16_16::from_int(100);
    let b = Fixed16_16::from_int(37);
    c.bench_function("Fixed16_16::fixed_div", |bencher| {
        bencher.iter(|| black_box(a).fixed_div(black_box(b)))
    });
}

fn bench_fixed_from_int_roundtrip(c: &mut Criterion) {
    c.bench_function("Fixed16_16 from_int/to_int roundtrip", |bencher| {
        bencher.iter(|| Fixed16_16::from_int(black_box(42)).to_int())
    });
}

fn bench_fixed_add_chain(c: &mut Criterion) {
    // Simulate a chain of additions like P_MovePlayer's thrust accumulation.
    let step = Fixed16_16::from_int(3);
    c.bench_function("Fixed16_16 add chain x1000", |bencher| {
        bencher.iter(|| {
            let mut acc = Fixed16_16::ZERO;
            for _ in 0..1000 {
                acc = acc + black_box(step);
            }
            black_box(acc)
        })
    });
}

fn bench_fixed_mul_by_one(c: &mut Criterion) {
    // Identity check — should be near-zero cost after inlining.
    let a = Fixed16_16::from_int(12345);
    let one = doom_types::FIXED_ONE;
    c.bench_function("Fixed16_16::fixed_mul by FIXED_ONE", |bencher| {
        bencher.iter(|| black_box(a).fixed_mul(black_box(one)))
    });
}

criterion_group!(
    fixed_benches,
    bench_fixed_mul,
    bench_fixed_div,
    bench_fixed_from_int_roundtrip,
    bench_fixed_add_chain,
    bench_fixed_mul_by_one
);
criterion_main!(fixed_benches);
