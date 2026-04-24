//! OPL2/3 FM synthesis, MUS→MIDI conversion, PCM SFX mixing,
//! positional audio, and cpal audio output driver.

mod driver;
mod midi;
mod mixer;
mod mus;
mod opl;
mod sfx;
mod sfx_mixer;
mod spatial;
mod wav;

pub use driver::{AudioDriver, SharedMidiPlayer, SharedSfxMixer};
pub use midi::{GenmidiBank, GenmidiInstrument, MidiPlayer};
pub use mixer::PcmSample;
pub use mus::{MusEvent, MusHeader, MusScore};
pub use sfx::{SfxCache, play_sfx};
pub use sfx_mixer::{MAX_CHANNELS, SfxChannel, SfxMixer, SfxPriority};
pub use spatial::{MAX_SFX_DIST, SfxEmitter, SpatialParams, compute_spatial};
pub use wav::{encode_pcm16_wav_mono, render_mus_to_wav_mono};

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
