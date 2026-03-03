//! Criterion benchmarks for the doom-renderer hot paths.
//!
//! Covers:
//! - `draw_column` — R_DrawColumn throughput (textured vertical strip)
//! - `draw_span`   — R_DrawSpan throughput (textured horizontal strip)
//! - `Framebuffer::clear` — full-framebuffer clear baseline
//! - `render_level` — full BSP traversal + perspective projection on a minimal level
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use doom_map::{Blockmap, Level, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex};
use doom_renderer::{
    DrawColumnParams, DrawSpanParams, Framebuffer, IDENTITY_COLORMAP, PaletteLut, draw_column,
    draw_span, render_level,
};
use doom_types::Bam;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal `Level` with one sector and one seg (identical to the
/// helper in `render.rs` tests).  Constructed once per benchmark group so
/// we're measuring render time, not level-construction time.
fn make_minimal_level() -> Level {
    let vertexes = vec![Vertex { x: 0, y: 128 }, Vertex { x: 128, y: 128 }];
    let sectors = vec![Sector {
        floor_height: 0,
        ceil_height: 128,
        floor_flat: *b"FLAT1\0\0\0",
        ceil_flat: *b"FLAT2\0\0\0",
        light_level: 192,
        special: 0,
        tag: 0,
    }];
    let sidedefs = vec![Sidedef {
        x_offset: 0,
        y_offset: 0,
        upper_texture: *b"WALL1\0\0\0",
        lower_texture: *b"WALL2\0\0\0",
        middle_texture: *b"WALL3\0\0\0",
        sector: 0,
    }];
    let linedefs = vec![Linedef {
        from_vertex: 0,
        to_vertex: 1,
        flags: 0,
        special: 0,
        tag: 0,
        right_sidedef: 0,
        left_sidedef: 0xFFFF,
    }];
    let segs = vec![Seg {
        from_vertex: 0,
        to_vertex: 1,
        angle: 0,
        linedef: 0,
        direction: 0,
        offset: 0,
    }];
    let ssectors = vec![Ssector {
        seg_count: 1,
        first_seg: 0,
    }];
    let things = vec![Thing {
        x: 0,
        y: 0,
        angle: 0,
        kind: 1,
        flags: 7,
    }];

    // Reject: 1 sector → ceil(1/8) = 1 byte
    let reject = Reject::parse_lump(&[0u8; 1], 1).expect("reject parse");

    // Minimal blockmap: 1×1 grid, one empty list
    let mut bm_data = vec![0u8; 8 + 2 + 4];
    bm_data[4..6].copy_from_slice(&1u16.to_le_bytes()); // x_count = 1
    bm_data[6..8].copy_from_slice(&1u16.to_le_bytes()); // y_count = 1
    bm_data[8..10].copy_from_slice(&5u16.to_le_bytes()); // offset to list
    bm_data[10..12].copy_from_slice(&0u16.to_le_bytes()); // sentinel
    bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes()); // terminator
    let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

    Level {
        name: "TEST".to_owned(),
        things,
        linedefs,
        sidedefs,
        vertexes,
        segs,
        ssectors,
        nodes: vec![],
        sectors,
        reject,
        blockmap,
    }
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

/// R_DrawColumn throughput: textured 200-pixel-tall column (full screen height).
fn bench_draw_column(c: &mut Criterion) {
    let mut fb = Framebuffer::new();

    // Texture: 128-texel column (power of 2), all zeroed (maps to colormap[0]).
    let source = [0u8; 128];

    let params = DrawColumnParams {
        x: 160, // center column
        y_top: 0,
        y_bot: 199, // full height = 200 pixels
        frac: 0,
        fracstep: 1 << 16, // 1 texel per pixel
        source: &source,
        colormap: &IDENTITY_COLORMAP,
    };

    c.bench_function("draw_column 200px textured", |bencher| {
        bencher.iter(|| draw_column(&mut fb, black_box(&params)))
    });
}

/// R_DrawSpan throughput: textured 320-pixel-wide floor span (full screen width).
fn bench_draw_span(c: &mut Criterion) {
    let mut fb = Framebuffer::new();

    // Flat texture: 64×64 palette indices, all zeroed.
    let source = [0u8; 4096];

    let params = DrawSpanParams {
        y: 100, // middle row
        x1: 0,
        x2: 319, // full width = 320 pixels
        ds_xfrac: 0,
        ds_yfrac: 0,
        ds_xstep: 1 << 16,
        ds_ystep: 1 << 16,
        source: &source,
        colormap: &IDENTITY_COLORMAP,
    };

    c.bench_function("draw_span 320px textured", |bencher| {
        bencher.iter(|| draw_span(&mut fb, black_box(&params)))
    });
}

/// Framebuffer::clear — baseline for "wipe the back-buffer each frame".
fn bench_framebuffer_clear(c: &mut Criterion) {
    let mut fb = Framebuffer::new();
    c.bench_function("Framebuffer::clear 320x200", |bencher| {
        bencher.iter(|| fb.clear(black_box(0)))
    });
}

/// Full render pipeline: BSP traversal + per-seg projection on a minimal level.
///
/// Player faces the single wall seg; exercises background fill, trig lookup,
/// near-clip, column projection, and Z-buffer in one shot.
fn bench_render_level_minimal(c: &mut Criterion) {
    // init_trig_tables is required for sin/cos used inside render_level.
    // SAFETY: single-threaded benchmark init.
    unsafe {
        Bam::init_trig_tables();
    }

    let level = make_minimal_level();
    let palette = PaletteLut::grayscale();
    let mut fb = Framebuffer::new();

    // Player at origin, facing north (ANG90) so the wall at y=128 is visible.
    let player_angle = Bam(0x4000_0000); // ANG90

    c.bench_function("render_level minimal (1 seg)", |bencher| {
        bencher.iter(|| {
            render_level(
                black_box(&level),
                black_box(0_i32),
                black_box(0_i32),
                black_box(player_angle),
                &mut fb,
                black_box(&palette),
                None,
                None,
                None,
                None,
                false,
            )
        })
    });
}

/// draw_column in a tight inner loop simulating 320 wall columns per frame.
fn bench_draw_column_320_columns(c: &mut Criterion) {
    let mut fb = Framebuffer::new();
    let source = [32u8; 128]; // non-zero texel so colormap is exercised

    c.bench_function("draw_column 320 columns full-height", |bencher| {
        bencher.iter(|| {
            for x in 0..320usize {
                let params = DrawColumnParams {
                    x,
                    y_top: 0,
                    y_bot: 199,
                    frac: 0,
                    fracstep: 0x8000, // half-texel step
                    source: &source,
                    colormap: &IDENTITY_COLORMAP,
                };
                draw_column(&mut fb, &params);
            }
            black_box(fb.as_slice()[0])
        })
    });
}

criterion_group!(
    renderer_benches,
    bench_draw_column,
    bench_draw_span,
    bench_framebuffer_clear,
    bench_render_level_minimal,
    bench_draw_column_320_columns,
);
criterion_main!(renderer_benches);
