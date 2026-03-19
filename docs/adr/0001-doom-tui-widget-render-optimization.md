# ADR 0001: doom-tui Widget Render Optimization

**Date:** 2026-03-18
**Status:** Accepted

## Context

`DoomFramebufferWidget::render()` is called at the monitor refresh rate (60–144 Hz) and is the
primary terminal render path for the halfblocks (`▀`) display mode.  Profiling by code inspection
revealed three avoidable costs in the inner loop of the original implementation:

1. **Division per cell** — `(cx * fb_w) / term_w` was computed inside the inner loop (~3 integer
   divisions per cell × 12,100 cells at 220×55 = 36,300 divisions/frame).
2. **Invariant branches inside inner loop** — `match self.scaling_mode` and `if self.ascii_mode`
   were evaluated per cell even though both values are constant for the lifetime of a single render.
3. **`buf.cell_mut()` overhead** — each call performed a `Position` type conversion, a bounds
   check, and a stride multiply, totalling 12,100 redundant operations per frame.
4. **Per-cell palette clamp** — `lut.get(pal, idx)` did `.min()` + `PLAYPAL_COLORS * pal`
   on each of 2 pixel lookups per cell; `pal` is frame-invariant.

## Benchmark Setup

Criterion benchmarks added in `crates/doom-tui/benches/widget_bench.rs`.
Machine: Windows 11, release build (`--release`).
Framebuffer: 320×200, varied palette indices (no uniform-field shortcuts).

Three representative terminal sizes:

| Label | Cols × Rows | Cells |
|-------|-------------|-------|
| small | 80×25 | 2,000 |
| medium | 160×50 | 8,000 |
| large | 220×55 | 12,100 |

## Baseline (before)

| Mode | 80×25 | 160×50 | 220×55 |
|------|-------|--------|--------|
| Nearest | 31.2 µs | 126.6 µs | 190.8 µs |
| Bilinear | 51.8 µs | 207.6 µs | 310.2 µs |

## Optimizations Applied

### Pass 1: Coordinate tables + palette pre-slice + hoisted match

- **`PaletteLut::palette_slice()`** added to `doom-renderer/src/palette.rs`: returns a `&[Rgb]`
  slice for the active palette. One call per render replaces 2 × `lut.get()` calls per cell
  (eliminates per-call `.min()` + multiply by `PLAYPAL_COLORS`).
- **Precomputed coordinate tables** — `x_map`, `y_top_map`, `y_bot_map` (Nearest) and `fx_map`,
  `fy_top_map`, `fy_bot_map` (Bilinear) computed once before the loops.  Tables are ≤330 `usize`
  entries (≤2.6 KB), fitting entirely in L1 cache.  Bilinear tables eliminate 3 × u64 divisions
  per cell; Nearest tables eliminate 3 usize divisions per cell (LLVM may already reciprocal-
  multiply these, so win is modest).
- **Row offset hoisted to outer loop** — `top_row_base = y_top_map[cy] * fb_w` computed once per
  row; the inner loop does only `top_row_base + fb_x` (addition, not multiply).
- **`match (scaling_mode, ascii_mode)` hoisted outside loops** — eliminates per-cell branches on
  frame-invariant values; produces four tight specialized loops instead of one polymorphic one.

| Mode | 80×25 | 160×50 | 220×55 |
|------|-------|--------|--------|
| Nearest | 30.3 µs (−2.2%) | 121.5 µs (−4.4%) | 183.9 µs (−3.6%) |
| Bilinear | 46.7 µs (−9.0%) | 186.9 µs (−9.6%) | 285.9 µs (−7.8%) |

Bilinear wins more because u64 division → table lookup is a larger relative saving than usize
division → table lookup (where LLVM may already use reciprocal multiplication).

### Pass 2: Direct `buf.content` slice access

`buf.cell_mut((area.x + cx, area.y + cy))` was replaced with direct `buf.content` slice access:

```rust
let buf_stride = buf.area.width as usize;
let area_origin_idx =
    (area.y - buf.area.y) as usize * buf_stride + (area.x - buf.area.x) as usize;

// Per row:
let row_start = area_origin_idx + cy * buf_stride;
let row_cells = &mut buf.content[row_start..row_start + term_w];
for (cell, &fb_x) in row_cells.iter_mut().zip(x_map.iter()) { ... }
```

This replaces 12,100 `cell_mut()` calls (each: `Position` conversion + `area.contains()` check +
stride multiply + `Option` wrapping) with one slice borrow per row (55 slice operations total).

`buf.content` is a `pub Vec<Cell>` in ratatui; `buf.area` is also public.  The computation is
equivalent to ratatui's own `index_of()`.

| Mode | 80×25 | 160×50 | 220×55 |
|------|-------|--------|--------|
| Nearest | 29.0 µs (−4.3%) | 114.3 µs (−5.9%) | 174.2 µs (−5.3%) |
| Bilinear | 45.3 µs (−3.0%) | 181.1 µs (−3.1%) | 281.0 µs (N/S) |

## Final Results vs Baseline

| Mode | 80×25 | 160×50 | 220×55 |
|------|-------|--------|--------|
| Nearest | 29.0 µs (**−7.1%**) | 114.3 µs (**−9.7%**) | 174.2 µs (**−8.7%**) |
| Bilinear | 45.3 µs (**−12.5%**) | 181.1 µs (**−12.7%**) | 281.0 µs (**−9.4%**) |

At 60 Hz, the large-terminal nearest render now consumes **10.5 ms/sec** (174.2 µs × 60) instead
of 11.4 ms/sec — saving ~1 ms/sec of CPU time at the largest common terminal size.

## Remaining Bottleneck

After these changes the dominant cost per cell is `Cell::set_char('▀') + set_fg + set_bg`.
`Cell` uses `compact_str::CompactString` for the symbol (inline for ≤24 bytes) and stores two
`Color` fields.  Per cell: ~3 bytes symbol write + 2 × 4-byte Color writes = ~11 bytes minimum,
totalling ~133 KB of writes for a 220×55 terminal.  Further improvement would require bypassing
ratatui's Cell API entirely and writing terminal escape sequences directly — a much larger change.

## Decision

Apply both passes.  The changes are:
- Correct: all 48 doom-tui tests and 2,494 workspace tests pass.
- Safe: `buf.content` and `buf.area` are public ratatui fields; the index formula matches
  ratatui's own `Buffer::index_of()`.
- Measurable: all improvements are statistically significant (p < 0.05) except bilinear 220×55
  pass 2 (no change detected, already dominated by bilinear sample cost).

## Files Changed

- `crates/doom-renderer/src/palette.rs` — added `PaletteLut::palette_slice()`
- `crates/doom-tui/src/widget.rs` — rewrote `Widget::render()` (see above)
- `crates/doom-tui/benches/widget_bench.rs` — new benchmark file
- `crates/doom-tui/Cargo.toml` — added `[[bench]]` entry for widget_bench
