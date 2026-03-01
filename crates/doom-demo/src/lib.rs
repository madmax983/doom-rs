//! LMP demo recording and byte-accurate playback.
//!
//! # Overview
//! - [`lmp`] — wire format types: [`LmpHeader`], [`LmpTicEntry`], [`DemoError`].
//! - [`record`] — [`DemoRecorder`]: accumulates tics and writes an LMP file.
//! - [`playback`] — [`DemoPlayer`]: parses an LMP file and replays tics.

pub mod lmp;
pub mod playback;
pub mod record;

pub use lmp::{DemoError, LmpHeader, LmpTicEntry, LMP_VERSION, DEMO_SENTINEL};
pub use playback::DemoPlayer;
pub use record::DemoRecorder;
