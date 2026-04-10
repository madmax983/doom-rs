//! Criterion benchmarks for column and span drawing.
use criterion::{Criterion, criterion_group, criterion_main};

/// Provides a baseline benchmark to validate the `criterion` setup.
///
/// Before profiling complex rendering kernels like `R_DrawColumn` or `R_DrawSpan`,
/// we need to ensure the benchmark harness itself isn't introducing catastrophic
/// overhead. This trivially measures the cost of invoking an empty loop iteration.
///
/// ## Examples
/// ```ignore
/// // cargo bench --bench column_span
/// ```
pub fn bench_placeholder(c: &mut Criterion) {
    c.bench_function("placeholder", |b| b.iter(|| 1 + 1));
}

criterion_group!(benches, bench_placeholder);
criterion_main!(benches);
