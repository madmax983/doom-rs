//! Windowed presentation layer for doom-rs, built on the `abrash` winit +
//! softbuffer backend.
//!
//! Doom's renderer writes a 320×200 8-bit palette-indexed
//! [`Framebuffer`](doom_renderer::Framebuffer). This crate presents that frame
//! in a native desktop window by:
//!
//! 1. expanding the paletted frame to `0xFFRRGGBB` ARGB via the canonical
//!    [`PaletteLut::expand_argb`](doom_renderer::PaletteLut::expand_argb),
//! 2. nearest-neighbour integer-scaling it to a vanilla 4:3 aspect
//!    ([`scale`]),
//! 3. driving the game at a fixed 35 Hz with a tic accumulator ([`tics`]) that
//!    reuses doom-tui's [`TIC_DURATION`](doom_tui::TIC_DURATION), and
//! 4. translating `winit` keyboard events into the crossterm codes doom-tui's
//!    `InputState` expects ([`input_adapter`]).
//!
//! The windowed host ([`host`]) is generic over doom-tui's
//! [`DoomApp`](doom_tui::DoomApp) trait, so the `doom-app` binary can pass its
//! own `DoomGame`. This crate does **not** depend on `doom-app`.
//!
//! The pure logic (scaling, key mapping, tic accumulation, palette expansion) is
//! unit-tested without opening a window; the thin windowing glue in [`host`] is
//! correct-by-construction.

pub mod host;
pub mod input_adapter;
pub mod scale;
pub mod tics;

pub use host::{DoomWindowApp, PresentError, run_windowed};
pub use input_adapter::winit_key_to_crossterm;
pub use scale::{fit_scale, scale_nearest_4_3};
pub use tics::TicAccumulator;
