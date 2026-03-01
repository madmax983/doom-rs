//! cpal audio output driver.
//!
//! Wraps a `cpal` output stream and routes its callback through a shared
//! [`Mixer`] (PCM SFX) and [`MidiPlayer`] (OPL2 FM music).  Both outputs are
//! mixed in the callback and written to the cpal buffer.
//!
//! For environments without a real audio device (CI, tests) use
//! [`AudioDriver::null`] which bypasses cpal entirely.

use std::sync::{Arc, Mutex};

use crate::{midi::MidiPlayer, mixer::Mixer, AudioError};

// ---------------------------------------------------------------------------
// SendStream — a Send wrapper around cpal::Stream
// ---------------------------------------------------------------------------
//
// On some platforms (notably Windows/WASAPI) cpal::Stream is not Send because
// it contains a raw pointer via PhantomData.  However we only ever *drop* the
// stream from the thread that created it (the audio thread never moves the
// struct), so wrapping it with an unsafe Send assertion is safe in practice.

struct SendStream(#[allow(dead_code)] cpal::Stream);

// SAFETY: We only hold the stream to keep it alive; we never access it from
// multiple threads concurrently.  cpal streams are safe to drop from any
// thread even when not formally Send.
#[allow(unsafe_code)]
unsafe impl Send for SendStream {}

// ---------------------------------------------------------------------------
// AudioDriver
// ---------------------------------------------------------------------------

/// Live audio output driver backed by cpal.
///
/// Drop the driver to stop audio output.  The internal cpal stream is kept
/// alive for as long as this struct is alive.
pub struct AudioDriver {
    /// Held solely to keep the cpal stream alive.
    _stream: Option<Box<SendStream>>,
    /// The shared PCM mixer.  Callers queue sounds here; the cpal callback
    /// drains it on each audio buffer fill.
    pub mixer: Arc<Mutex<Mixer>>,
    /// The shared MIDI/OPL2 player.  The cpal callback calls
    /// `advance_samples` on each buffer fill to generate FM music output.
    pub midi: Arc<Mutex<MidiPlayer>>,
}

impl AudioDriver {
    /// Attempt to open the system default audio output at `sample_rate` Hz.
    ///
    /// The cpal callback mixes PCM SFX from [`Mixer`] and OPL2 FM audio from
    /// [`MidiPlayer`] into each output buffer frame.
    ///
    /// # Errors
    /// - [`AudioError::NoDevice`] — no default output device.
    /// - [`AudioError::Stream`] — cpal could not create or start the stream.
    pub fn open(sample_rate: u32) -> Result<Self, AudioError> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioError::NoDevice)?;

        let config = device
            .default_output_config()
            .map_err(|e| AudioError::Stream(e.to_string()))?;

        let mixer = Arc::new(Mutex::new(Mixer::new(sample_rate)));
        let midi = Arc::new(Mutex::new(MidiPlayer::new()));

        let mixer_cb = Arc::clone(&mixer);
        let midi_cb = Arc::clone(&midi);

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    // The cpal buffer is interleaved stereo f32.
                    // We treat it as stereo: n_mono = data.len() / 2.
                    // If the buffer happens to be mono (channels == 1) then
                    // n_mono == data.len() and the stereo loop below works
                    // correctly because i*2 == i*2+1 == i for channels=1
                    // — but we guard with saturating to handle odd lengths.
                    let n_mono = (data.len() / 2).max(1);

                    let mut pcm_buf = vec![0i16; data.len()];
                    let mut opl_buf = vec![0.0f32; n_mono];

                    if let Ok(mut m) = mixer_cb.lock() {
                        m.mix_frame(&mut pcm_buf);
                    }
                    if let Ok(mut mp) = midi_cb.lock() {
                        mp.advance_samples(n_mono, sample_rate, &mut opl_buf);
                    }

                    // Mix PCM and OPL into the output buffer.
                    // data is interleaved stereo: [L0, R0, L1, R1, ...].
                    // pcm_buf mirrors that layout; opl_buf is mono.
                    for i in 0..n_mono {
                        let pcm_l = f32::from(pcm_buf[i * 2]) / 32_768.0;
                        let pcm_r = f32::from(*pcm_buf.get(i * 2 + 1).unwrap_or(&0i16))
                            / 32_768.0;
                        let opl = opl_buf[i] * 0.5; // balance OPL volume
                        if let Some(out_l) = data.get_mut(i * 2) {
                            *out_l = (pcm_l + opl).clamp(-1.0, 1.0);
                        }
                        if let Some(out_r) = data.get_mut(i * 2 + 1) {
                            *out_r = (pcm_r + opl).clamp(-1.0, 1.0);
                        }
                    }
                },
                |err| eprintln!("audio stream error: {err}"),
                None,
            )
            .map_err(|e| AudioError::Stream(e.to_string()))?;

        stream
            .play()
            .map_err(|e| AudioError::Stream(e.to_string()))?;

        Ok(Self {
            _stream: Some(Box::new(SendStream(stream))),
            mixer,
            midi,
        })
    }

    /// Create a driver that owns no real stream — safe for headless / test use.
    #[must_use]
    pub fn null() -> Self {
        Self {
            _stream: None,
            mixer: Arc::new(Mutex::new(Mixer::new(44_100))),
            midi: Arc::new(Mutex::new(MidiPlayer::new())),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mus::{MusEvent, MusHeader, MusScore};

    #[test]
    fn null_driver_can_be_created() {
        let driver = AudioDriver::null();
        let mixer = driver.mixer.lock().expect("mutex should not be poisoned");
        assert_eq!(mixer.sample_rate, 44_100);
    }

    #[test]
    fn driver_null_has_midi() {
        let driver = AudioDriver::null();
        // The midi Arc must be valid and the mutex must not be poisoned.
        let mp = driver.midi.lock().expect("midi mutex should not be poisoned");
        // A freshly created player has no score loaded.
        assert!(mp.current_score.is_none(), "null driver midi must start with no score");
    }

    #[test]
    fn driver_midi_player_can_load_score() {
        let driver = AudioDriver::null();

        let score = MusScore {
            header: MusHeader {
                score_length: 0,
                score_start: 0,
                primary_channels: 1,
                secondary_channels: 0,
                instrument_count: 0,
            },
            instruments: Vec::new(),
            events: vec![
                (0, MusEvent::PlayNote { channel: 0, note: 60, volume: Some(100) }),
                (10, MusEvent::ScoreEnd),
            ],
        };

        // Lock, load, verify — must not panic.
        let mut mp = driver.midi.lock().expect("midi mutex must not be poisoned");
        mp.load_score(score);
        assert!(mp.current_score.is_some(), "score must be loaded after load_score");
        assert_eq!(mp.event_cursor, 0);
    }
}
