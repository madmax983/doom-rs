#![allow(missing_docs)]

//! Criterion benchmarks for the palette-aware Sixel encoder.
//!
//! Run with:
//!   cargo bench -p doom-tui
//!
//! The three scenarios mirror real terminal sizes:
//!   - small:    640×400   (~small window or high-DPI at 2× zoom)
//!   - medium:  1280×800   (~typical 160-col terminal)
//!   - full:    1760×1100  (~maximised 220-col terminal with 8×20 font)
//!
//! FPS headroom = 1000 ms / bench_time_ms.  Goal: > 60 FPS at "full".

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use doom_renderer::PaletteLut;
use doom_tui::encode_doom_sixel;

const SRC_W: usize = 320;
const SRC_H: usize = 200;

/// Build a pseudo-realistic framebuffer: sky strip, walls, floor, a few
/// sprites — enough colour variety to stress the encoder without loading a WAD.
fn make_framebuffer() -> Vec<u8> {
    let mut data = vec![0u8; SRC_W * SRC_H];
    for y in 0..SRC_H {
        for x in 0..SRC_W {
            let idx: u8 = match y {
                0..=39 => {
                    // Sky: horizontal gradient
                    ((x * 4) / SRC_W) as u8 + 160
                }
                40..=119 => {
                    // Walls: checkerboard of ~16 colours
                    let tx = x / 32;
                    let ty = (y - 40) / 16;
                    ((tx + ty * 5) % 48 + 64) as u8
                }
                120..=167 => {
                    // Floor: diagonal pattern
                    (((x + y) / 8) % 32 + 32) as u8
                }
                _ => {
                    // HUD / status bar: solid bands
                    (y % 16) as u8
                }
            };
            data[y * SRC_W + x] = idx;
        }
    }
    data
}

fn bench_sixel_encoder(c: &mut Criterion) {
    let lut = PaletteLut::grayscale();
    let data = make_framebuffer();

    let scenarios: &[(&str, usize, usize, u16, u16)] = &[
        // (label,  dst_w, dst_h, area_w, area_h)
        ("small_640x400", 640, 400, 80, 25),
        ("medium_1280x800", 1280, 800, 160, 50),
        ("full_1760x1100", 1760, 1100, 220, 55),
    ];

    let mut group = c.benchmark_group("sixel_encode");
    for &(label, dst_w, dst_h, area_w, _area_h) in scenarios {
        group.bench_with_input(
            BenchmarkId::new("encode", label),
            &(dst_w, dst_h),
            |b, &(dw, dh)| {
                b.iter(|| encode_doom_sixel(&data, &lut, 0, SRC_W, SRC_H, dw, dh, area_w));
            },
        );
    }
    group.finish();
}

/// Benchmark encoded output size (bytes) — printed as a custom metric so you
/// can see how much data is being pushed to the terminal per frame.
fn bench_sixel_output_size(c: &mut Criterion) {
    let lut = PaletteLut::grayscale();
    let data = make_framebuffer();

    let mut group = c.benchmark_group("sixel_output_size");
    group.bench_function("full_1760x1100_bytes", |b| {
        b.iter(|| {
            let s = encode_doom_sixel(&data, &lut, 0, SRC_W, SRC_H, 1760, 1100, 220);
            std::hint::black_box(s.len())
        });
    });
    group.finish();
}

criterion_group!(benches, bench_sixel_encoder, bench_sixel_output_size);
criterion_main!(benches);
