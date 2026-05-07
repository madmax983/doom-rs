#![allow(missing_docs)]

//! Criterion benchmarks for column and span drawing.
use criterion::{Criterion, criterion_group, criterion_main};

/// Placeholder benchmark function.
pub fn bench_placeholder(c: &mut Criterion) {
    c.bench_function("placeholder", |b| b.iter(|| 1 + 1));
}

criterion_group!(benches, bench_placeholder);
criterion_main!(benches);
