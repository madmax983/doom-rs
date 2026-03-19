# ADR 0002: Sixel Blit Optimization — Dirty-Frame Cache + Async Blit Thread

**Date:** 2026-03-18
**Status:** Accepted

## Context

In sixel mode, per-frame timings measured via the `on_frame_timings` hook showed:

| Phase | Avg | Notes |
|-------|-----|-------|
| tick | ~25 µs | negligible |
| render | ~767 µs | fast |
| blit | ~10 ms (title) / ~28–41 ms (gameplay) | **97% of frame time** |

The bottleneck is **not** the sixel encoder (benchmarked at ~4 ms).
It is the synchronous `terminal.draw()` call which writes the sixel payload through the pty and
blocks until the terminal application finishes rendering it.

Two structural inefficiencies compound the problem:

1. **Tic boundary gap** — Doom simulates at 35 Hz; the render loop runs at monitor Hz (60–144 Hz).
   Between tics the framebuffer is identical to the previous frame.  At 60 Hz that is 25/60 ≈ 42%
   of frames that produce the exact same sixel image, each paying the full ~28 ms pty-write cost.

2. **Static scenes** — During the title screen, intermission, and paused states the framebuffer never
   changes.  Every frame paid ~10 ms for an identical pty write.

3. **Main-loop blocking** — Even during active gameplay (28–41 ms blit), the synchronous
   `terminal.draw()` blocked `tick()` + `render()` + input polling, compressing effective
   game-loop time and causing frame-rate drops to 24 fps during complex scenes.

## Solution: Two-pass optimization

### Pass 1 — Dirty-frame cache (pre-encode on main thread)

**Observation:** ratatui diffs at the cell-symbol level before any pty I/O.  If the sixel string
stored in cell (0, 0) is byte-for-byte identical to the previous frame, ratatui emits **zero bytes**
for that cell — only the status bar (~50 bytes) is flushed.

**Implementation:**

The sixel string is pre-encoded *before* `terminal.draw()` (working around the borrow checker, which
prevents reading `self.sixel_cache` while `self.terminal` is mutably borrowed by `draw()`).

Dirty conditions (any triggers a re-encode):

- `active_palette` changed
- Terminal dimensions changed (window resize)
- `fb.as_slice() != sixel_prev_fb` (memcmp of 64,000 bytes)

On clean frames: `sixel_cache.clone()` is sent; ratatui emits zero pty bytes for the game cell.

| Scenario | Before | After (pass 1 only) |
|----------|--------|---------------------|
| Title screen / paused | ~10 ms | ~0.5 ms |
| Between-tic frames at 60 Hz | ~28 ms | ~0.5 ms |
| Active gameplay | ~28–41 ms | ~28–41 ms (unchanged) |

### Pass 2 — Async blit thread

**Observation:** even with the dirty-frame cache, active gameplay still blocked the main loop for
28–41 ms per frame.  The fix is to decouple terminal I/O from game simulation entirely.

**Architecture:**

```
Main thread                          Blit thread
──────────                           ───────────
poll_events()         ┐              owns Terminal<CrosstermBackend<Stdout>>
drain_ready_tics()    │ ~1 ms        receives BlitFrame via sync_channel(1)
app.render()          │              calls terminal.draw()    ~28 ms
encode_sixel (cache)  ┘              stores elapsed → AtomicU64
try_send(BlitFrame)   <── channel ──>
continue immediately                 loop
```

- `Terminal` is moved into the blit thread on the first `run()` call.
  The main thread never writes to stdout after that point.
- `sync_channel(1)`: capacity-1 bounded channel.  `try_send` is non-blocking.
  If the channel is full (blit still writing), the new frame is silently dropped.
  The display rate is naturally limited to the terminal's render throughput.
- `Arc<AtomicU64>` feeds the blit thread's `terminal.draw()` elapsed time back to the main
  thread for status-bar display and `on_frame_timings` reporting (one frame stale — acceptable).

**Data sent per frame:**

| Protocol | Payload | Size |
|----------|---------|------|
| Halfblocks | `Framebuffer::clone()` + `active_palette` + `scaling_mode` | 64 KB |
| Sixel | `String` (pre-encoded) | varies, ~5–400 KB |
| Kitty / iTerm2 | `DynamicImage` (RGB converted on main thread) | 192 KB |

**Frame-drop semantics:**

When the blit thread is busy (sixel: ~28 ms per frame, 35 Hz sim rate → ~98% busy), the main
thread's `try_send` returns `Err(Full)` and the frame is dropped.  The next frame will be sent
when the blit thread finishes.  Effective display rate = min(sim rate, terminal render rate) — which
matches the pre-refactor behavior but without blocking the main thread.

## Combined impact

| Scenario | Before both passes | After both passes |
|----------|--------------------|-------------------|
| Title screen / paused | ~10 ms blit, blocks main | ~0 ms (dirty skip) |
| Between-tic frames at 60 Hz | ~28 ms blit, blocks main | ~0 ms (dirty skip) |
| Active gameplay (moving) | ~28–41 ms blit, **blocks** main | ~28–41 ms on blit thread, **main free** |
| Main-loop blocked per second | ~980 ms/sec (35 frames × 28 ms) | **~0 ms** |

The game loop (tick, render, input) now runs without terminal I/O backpressure.
Simulation at 35 Hz is maintained even when the terminal can only render 24–35 fps of sixel.

## Remaining bottleneck

The raw pty write latency (~24 ms of the ~28 ms blit) is terminal-side rendering time and cannot
be reduced further without:

- **Delta sixel**: only encoding changed rectangular regions
- **Lower resolution output**: fewer destination pixels → smaller sixel payload → less RLE
- **A faster terminal**: e.g., WezTerm or foot process sixel faster than Windows Terminal

These are out of scope for this change.

## Files Changed

- `crates/doom-renderer/src/palette.rs` — added `#[derive(Clone)]` to `PaletteLut`
- `crates/doom-tui/src/event_loop.rs` — async blit thread + dirty-frame cache (complete refactor)
- `docs/adr/0002-sixel-dirty-frame-cache.md` — this document
