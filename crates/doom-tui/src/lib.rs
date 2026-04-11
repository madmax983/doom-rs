//! Ratatui + crossterm terminal frontend for Doom.
//!
//! Converts a palette-indexed `Framebuffer` to `▀` half-block terminal cells.
//! Decouples the fixed 35 tic/sec game simulation from the monitor render rate.
//!
//! # Key types
//! - [`DoomEventLoop`] — manage the terminal, run the game loop
//! - [`DoomFramebufferWidget`] — ratatui widget, palette → RGB at blit time
//! - [`InputState`] / [`TicInput`] — input synthesis from held keys
//! - [`DoomApp`] — implement this trait to plug your game in

pub(crate) mod charset;
pub(crate) mod cogmind;
pub(crate) mod event_loop;
pub(crate) mod input;
pub(crate) mod scaler;
pub(crate) mod sixel;
pub(crate) mod widget;

pub use charset::{CharSet, RendererMode};
pub use cogmind::{CogmindCell, CogmindFrame, CogmindHud, CogmindHudWidget, CogmindWidget};
pub use event_loop::{DoomApp, DoomEventLoop, EventLoopError, TIC_DURATION, TIC_RATE_HZ};
pub use input::{InputState, TicInput, buttons};
pub use scaler::{ScalingMode, sample_bilinear};
pub use widget::DoomFramebufferWidget;

pub use sixel::{DoomSixelWidget, encode_doom_sixel};
