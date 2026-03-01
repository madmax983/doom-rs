//! Criterion benchmarks for column and span drawing.
use criterion::{criterion_group, criterion_main, Criterion};

pub fn bench_placeholder(c: &mut Criterion) {
    c.bench_function("placeholder", |b| b.iter(|| 1 + 1));
}

criterion_group!(benches, bench_placeholder);
criterion_main!(benches);
