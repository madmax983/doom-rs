# doom-rs ← abrash: Presentation-Layer Integration Design

> Design doc only — **no integration code is written yet.** This spec records the
> two codebases' seams, what `abrash` actually provides, the real gap, and a
> step-by-step plan to plug `abrash` in as doom-rs's **windowed presentation
> layer**. It implements decision §6.2 of [`PARITY_ROADMAP.md`](PARITY_ROADMAP.md).
>
> _Last updated: 2026-07-08._

## 0. Summary

`abrash` (`madmax983/abrash`) is a Rust software-rasterization workspace with a
`winit`+`softbuffer` windowed backend. doom-rs is a Rust terminal Doom engine
whose renderer writes a **320×200 8-bit palette-indexed** framebuffer. The two
are already closely related — **doom-rs's terminal event loop was ported from
`abrash`'s TUI backend** (`abrash/src/platform/tui.rs`), and both are Rust
edition 2024 on the same `ratatui` 0.30 / `crossterm` 0.29 stack.

The headline finding: **`abrash` does not "plop in" as a Doom *render backend*,
because `abrash` is RGBA-only and Doom is paletted — but it does not need to.**
Doom's BSP software renderer is self-contained and must keep writing palette
indices (colormaps, lighting, palette flashes, and demo determinism all live in
index space). What `abrash` provides that doom-rs lacks is a **windowed
*presenter*** (`winit` window + `softbuffer` surface). Wiring that in requires a
thin **palette→RGBA expansion shim** (which doom-rs already performs today for
its Kitty/sixel terminal paths) plus a `winit`-driven **fixed-step host** that
mirrors doom-tui's existing 35 Hz tic accumulator. No FFI, no language boundary.

## 1. doom-rs seams

Three seams matter; all are concrete, named types.

### 1.1 Render-target seam
- **`doom_renderer::Framebuffer`** — `crates/doom-renderer/src/framebuffer.rs`.
  A 320×200 (`FB_SIZE = 64000`) `Box<[u8; FB_SIZE]>` of **palette indices**
  (0–255), row-major, top-left origin. `Framebuffer::width()/height()` are
  `const` (320/200). This is Doom's canonical render target and stays paletted.
- **`doom_renderer::PaletteLut`** — `crates/doom-renderer/src/palette.rs`.
  Precomputed `[palette_idx][color_idx] → Rgb` built from the WAD `PLAYPAL`
  lump (`from_playpal`); 14 palettes (`PLAYPAL_COUNT`). Lookup via
  `get(palette, color) -> Rgb` or `palette_slice(palette) -> &[Rgb]`. **This is
  the palette-expansion primitive** any presenter uses to turn indices into RGB.
- **`doom_tui::DoomApp::render(&mut self, fb: &mut Framebuffer)`** —
  `crates/doom-tui/src/event_loop.rs`. The trait method by which the game hands
  a finished 8-bit frame to the presentation layer. Implemented by `DoomGame` in
  `crates/doom-app/src/main.rs:1280`.
- **Concrete present/blit today:** `crates/doom-tui/src/event_loop.rs`
  (`DoomEventLoop::blit`, `run_blit_thread`), `crates/doom-tui/src/widget.rs`
  (`DoomFramebufferWidget`, palette→RGB at blit time), `sixel.rs`, `scaler.rs`.
  The Kitty/iTerm2 path in `blit()` already expands `fb → Vec<u8>` RGB via
  `lut.get(active_palette, idx)`; the sixel path does the same. **This existing
  expansion is exactly the shim an `abrash` presenter needs**, retargeted to u32.

### 1.2 Input seam
- **`crates/doom-tui/src/input.rs`** — `InputState` (raw held-key tracking) and
  `TicInput` (per-tic synthesized command: `forward_move: i8`,
  `side_move: i8`, `angle_turn: i16`, `buttons: u8`), produced by
  `InputState::to_tic_input()`.
- Events are pumped from **crossterm** in `DoomEventLoop::poll_events()`
  (`event_loop.rs`). A windowed path must feed `InputState` from **`winit`
  `WindowEvent`s** instead — a new adapter analogous to `poll_events`.

### 1.3 Timer seam
- **`crates/doom-tui/src/event_loop.rs`** — `TIC_RATE_HZ = 35`, `TIC_DURATION`
  (≈28.571 ms), a `tic_accumulator: Duration`, and `drain_ready_tics()` which
  advances `DoomApp::tick(TicInput)` at a fixed 35 Hz decoupled from the render
  rate; `target_frame_time` comes from `query_refresh_rate()`.
- **`DoomApp::tick(&mut self, input: TicInput)`** is the per-tic simulation
  callback (`DoomGame::tick`, `main.rs:982`).

### 1.4 App-level wiring
`crates/doom-app/src/main.rs` builds a `PaletteLut` (`blit_palette`), constructs
`DoomEventLoop::new()`, and calls `.run(&mut app, &blit_palette)` (main.rs
~2955–3040). This `run(...)` call is the single point a windowed host would
swap in for.

## 2. What abrash provides

Repo `madmax983/abrash` — a Cargo **workspace** (Rust edition 2024). Relevant
crates and API:

### 2.1 Framebuffer / surface model
- **`abrash_core::framebuffer::Framebuffer`** — a `Vec<u32>` in **`0xAARRGGBB`**
  (ARGB) row-major; `new(w, h)`, `as_slice()/as_mut_slice()`, `width()/height()`,
  `clear(u32)`. **32-bit RGBA only — no palette / indexed mode.**
- **`abrash_render::render_api::RenderTarget`** — framebuffer + z-buffer (+
  optional Hi-Z), the output surface for `abrash`'s 3D CPU rasterizer.
- **`abrash_render::render_api::BorrowedRenderTarget`** — wraps **caller-owned**
  `&mut [u32]` + `&mut [f32]` slices for **zero-copy** rendering into a window's
  buffer.

### 2.2 Palette / 8-bit indexed color — **CRITICAL**
- **`abrash` has no indexed-color surface.** Everything is 32-bit RGBA
  (`0xAARRGGBB`), and `abrash_core::color::Color` is a **linear-space f32 RGBA**
  with `from_argb_u32`/`to_argb_u32` bridges. There is no 256-entry palette
  concept anywhere in the API.
- **Consequence:** the palette must be resolved on doom-rs's side (via
  `PaletteLut`) *before* handing pixels to `abrash`. `abrash` receives already-
  expanded RGBA.

### 2.3 Input handling
- Windowed backend delivers raw **`winit::event::WindowEvent`** via the
  `WindowApp::input(&mut self, ctx, &WindowEvent)` callback
  (`abrash/src/platform/winit.rs`). Keyboard + mouse come through winit. There is
  also a minimal `platform::Event` enum (`Close`, `Resize`).

### 2.4 Timing / vsync / frame pacing
- **`abrash::platform::winit::FrameClock`** — `tick() -> f32` delta seconds; a
  **variable-rate** clock. `run_windowed()` drives `WindowApp::update` +
  `render` per redraw. **There is no fixed-step / accumulator** built in — the
  35 Hz tic discipline is doom-rs's responsibility (and doom-tui already has it).

### 2.5 Platforms / backends
Feature-gated in the root `abrash` crate:
- `backend-winit` *(default)* — `winit` window + `softbuffer` CPU present via
  **`SoftwarePresenter::present(&Framebuffer)`** (expands `0xAARRGGBB` →
  softbuffer `0x00RRGGBB` internally), `WindowApp` trait, `run_windowed()`.
- `backend-tui` — `crossterm` + `ratatui` (the code doom-tui was ported from).
- `backend-wasm` — `ratzilla`.
- `gpu-render` / `gpu-engine-compare` — `wgpu` (via `abrash-gpu-render`), Bevy/Fyrox compare.

### 2.6 Language / FFI story
- **Pure Rust, edition 2024** — same edition as doom-rs; `ratatui` 0.30,
  `crossterm` 0.29 match doom-tui. Integration is a **Cargo path/git dependency**;
  **no FFI, no C, no bindings.**

## 3. The gap — does abrash "plop in"?

**Not as a render backend; yes as a presenter, behind a small shim.** Three
mismatches, none fatal:

1. **Paletted (Doom) vs RGBA (abrash) — the real one.** `abrash` has no indexed
   surface, so it **cannot be Doom's render target**. Doom's renderer must keep
   emitting 8-bit indices (colormaps/lighting/palette-flash/demo determinism
   depend on it). The fix is not to change Doom's renderer but to **expand
   `Framebuffer` (`[u8; 64000]`) + `PaletteLut` → RGBA `u32`** at present time —
   the exact operation `DoomEventLoop::blit()` already does for Kitty/sixel,
   retargeted from `[r,g,b]` bytes to packed `0xFFRRGGBB`. `abrash`'s entire 3D
   pipeline (rasterizer, z-buffer, post-fx) is **unused** for Doom; only its
   platform/presenter subset is.

2. **Language/FFI — none.** Both Rust, both edition 2024, overlapping dep
   versions. This is the easy axis: add `abrash` as a workspace/git dependency
   with `default-features = false, features = ["backend-winit"]`.

3. **Timing model — variable vs fixed-step.** `abrash`'s `FrameClock`/
   `run_windowed` is variable-rate; Doom needs a fixed 35 Hz tic. doom-tui
   **already solved this** with its accumulator. A windowed host must port that
   accumulator into a `WindowApp::update` (advance `drain_ready_tics`), presenting
   in `WindowApp::render`, and adapt winit `WindowEvent`s into `InputState`.

**Biggest missing piece:** there is no shared paletted-surface abstraction, so
the palette-expansion shim + a winit-driven fixed-step host are net-new (though
small, and largely a re-shaping of code that already exists in doom-tui). A
secondary note: `abrash`'s own `backend-tui` and doom-tui's terminal loop are
near-duplicates (common ancestor), so the *net-new value* of this integration is
the **windowed** path, not the terminal path.

## 4. Integration plan (windowed presenter)

Stepwise, smallest-blast-radius first. **Do not** touch doom-rs's renderer,
playsim, or palette internals.

1. **Add the dependency.** Add `abrash` (git/path) to the workspace with
   `default-features = false, features = ["backend-winit"]`. Confirm edition-2024
   / `ratatui` 0.30 / `crossterm` 0.29 resolve cleanly against doom-rs's lockfile.
2. **Palette-expansion shim.** New function (e.g. in a `doom-tui` `abrash`
   presenter module or a new `doom-present` crate):
   `expand(fb: &Framebuffer, lut: &PaletteLut, active_palette: usize) -> Vec<u32>`
   producing `0xFFRRGGBB`. Reuse `PaletteLut::palette_slice` for the tight loop.
   Unit-test against a known PLAYPAL sample.
3. **abrash `Framebuffer` bridge.** Wrap the expanded `Vec<u32>` in an
   `abrash_core::framebuffer::Framebuffer` (320×200) and present via
   `SoftwarePresenter::present`, **or** write directly into a
   `BorrowedRenderTarget` / softbuffer buffer for zero-copy. Prefer the
   `SoftwarePresenter` path first (simplest); optimize to borrowed/zero-copy later.
4. **Winit input adapter.** Map `winit::event::WindowEvent` key/mouse events onto
   the existing `InputState` API (`key_down`/`key_up`/menu/console helpers),
   mirroring `DoomEventLoop::poll_events`. Keep `TicInput` synthesis unchanged.
5. **Fixed-step windowed host.** Implement `abrash::platform::winit::WindowApp`
   for a new `WindowedDoomHost` that owns the 35 Hz accumulator (port
   `tic_accumulator` + `drain_ready_tics` from `event_loop.rs`): drive
   `DoomApp::tick` in `update`, expand+present in `render`, feed input in
   `input`. Integer-scale 320×200 to the window size (nearest, matching
   `ScalingMode::Nearest`) — reuse `scaler.rs` logic.
6. **Selection at startup.** Add a presentation-backend switch in
   `doom-app/src/main.rs` (e.g. `--present terminal|window`) that chooses between
   the existing `DoomEventLoop::run` and the new windowed host. Terminal stays
   the default; windowed is opt-in.
7. **Parity guardrails.** Assert the windowed path calls `DoomApp::tick` at
   exactly 35 Hz and never advances the game `M_Random` index off render frames
   (protects demo determinism — the M1 concern). Add a headless/smoke test that
   drives N tics and checks the expanded RGBA against a golden frame.

**Explicitly out of scope for this plan:** using `abrash`'s 3D rasterizer,
z-buffer, GPU (`wgpu`) path, or post-processing for Doom; changing Doom's
paletted renderer; DOS-accurate input timing.

## 5. Open items for the user
- **Where does the shim live?** Options: a module inside `doom-tui`, or a new
  `doom-present`/`doom-window` crate. (Default assumption: a new crate keeps the
  winit dependency out of the terminal build.)
- **Dependency form:** vendor `abrash` as a path dependency, a git dependency
  pinned to a commit, or publish it. (Default assumption: git dependency pinned
  to a commit.)
- **Scaling/aspect:** integer nearest scale vs. 320×200 → 4:3 correction (Doom's
  non-square pixels). (Default assumption: integer nearest first; aspect
  correction later.)
