//! Criterion benchmarks for the half-block `▀` framebuffer widget.
//!
//! Run with:
//!   cargo bench -p doom-tui --bench widget_bench
//!
//! Three terminal sizes mirror real usage:
//!   - small:   80×25  (narrow window)
//!   - medium: 160×50  (typical 160-col terminal)
//!   - large:  220×55  (maximised terminal)
//!
//! The framebuffer is filled with a varied pattern (not uniform) to avoid
//! any unrealistic cache behaviour.
#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use doom_renderer::{Framebuffer, PaletteLut};
use doom_tui::DoomFramebufferWidget;
use doom_tui::scaler::ScalingMode;
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

fn make_framebuffer() -> Framebuffer {
    let mut fb = Framebuffer::new();
    // Varied palette indices — avoids uniform-field shortcuts in the compiler.
    for y in 0..Framebuffer::height() {
        for x in 0..Framebuffer::width() {
            fb.set_pixel(
                x,
                y,
                ((x.wrapping_mul(3).wrapping_add(y.wrapping_mul(7))) % 256) as u8,
            );
        }
    }
    fb
}

fn bench_widget_render(c: &mut Criterion) {
    let lut = PaletteLut::grayscale();
    let fb = make_framebuffer();

    let scenarios: &[(&str, u16, u16)] = &[
        ("small_80x25", 80, 25),
        ("medium_160x50", 160, 50),
        ("large_220x55", 220, 55),
    ];

    let mut group = c.benchmark_group("widget_render");

    for &(label, cols, rows) in scenarios {
        let area = Rect::new(0, 0, cols, rows);

        group.bench_with_input(BenchmarkId::new("nearest", label), &(cols, rows), |b, _| {
            b.iter(|| {
                let mut buf = Buffer::empty(area);
                DoomFramebufferWidget::new(&fb, &lut, 0).render(area, &mut buf);
                std::hint::black_box(buf)
            });
        });

        group.bench_with_input(
            BenchmarkId::new("bilinear", label),
            &(cols, rows),
            |b, _| {
                b.iter(|| {
                    let mut buf = Buffer::empty(area);
                    DoomFramebufferWidget::new(&fb, &lut, 0)
                        .with_scaling(ScalingMode::Bilinear)
                        .render(area, &mut buf);
                    std::hint::black_box(buf)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_widget_render);
criterion_main!(benches);
