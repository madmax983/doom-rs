//! OPL2/3 FM synthesis, MUS→MIDI conversion, PCM SFX mixing,
//! positional audio, and cpal audio output driver.

pub mod driver;
pub mod midi;
pub mod mixer;
pub mod mus;
pub mod opl;
pub mod sfx;
pub mod sfx_mixer;
pub mod spatial;

pub use driver::AudioDriver;
pub use midi::{GenmidiBank, GenmidiInstrument, MidiPlayer};
pub use mixer::{Mixer, PcmSample};
pub use mus::{MusEvent, MusScore};
pub use sfx::{SfxCache, SfxPriority, play_sfx};
pub use sfx_mixer::{MAX_CHANNELS, SfxChannel, SfxMixer, SfxPriority as SfxPriorityLevel};
pub use spatial::{MAX_SFX_DIST, SfxEmitter, SpatialParams, compute_spatial};

/// Top-level error type for the doom-audio crate.
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    /// No audio output device is available on this system.
    #[error("no audio output device available")]
    NoDevice,
    /// A cpal stream-level error occurred.
    #[error("cpal stream error: {0}")]
    Stream(String),
    /// The raw SFX lump data was malformed.
    #[error("invalid SFX lump: {0}")]
    InvalidSfx(&'static str),
    /// The raw MUS data was malformed.
    #[error("invalid MUS data: {0}")]
    InvalidMus(&'static str),
}
