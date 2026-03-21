//! cpal audio output driver.
//!
//! Welcome, weary traveler, to the final stage of our auditory pipeline.
//! Here we speak to the machine itself using the `cpal` library.
//!
//! The `AudioDriver` acts as the conductor of our metallic orchestra. It
//! wraps the OS-level stream, spinning up a high-priority background thread
//! that relentlessly demands audio data.
//!
//! To feed this beast, the driver holds shared references to our two distinct
//! ensembles:
//! - The [`SfxMixer`], which handles all PCM sound effects (demon roars, shotgun blasts)
//! - The [`MidiPlayer`], which synthesizes FM OPL2 music (the heavy metal soundtrack)
//!
//! In the `cpal` callback, these two streams are mixed together and blasted into
//! the buffer. They remain totally independent until the very last millisecond
//! before hitting the speakers.
//!
//! For environments without a real audio device (like a lonely CI server), use
//! [`AudioDriver::null`] to skip the hardware entirely.

#[cfg(feature = "loom")]
use loom::sync::{Arc, Mutex};
#[cfg(not(feature = "loom"))]
use std::sync::{Arc, Mutex};

use crate::{AudioError, midi::MidiPlayer, sfx_mixer::SfxMixer};

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
/// A thread-safe, shared reference to the [`SfxMixer`].
pub type SharedSfxMixer = Arc<Mutex<SfxMixer>>;

/// A thread-safe, shared reference to the [`MidiPlayer`].
pub type SharedMidiPlayer = Arc<Mutex<MidiPlayer>>;

/// Live audio output driver backed by cpal.
///
/// Drop the driver to stop audio output.  The internal cpal stream is kept
/// alive for as long as this struct is alive.
pub struct AudioDriver {
    /// Held solely to keep the cpal stream alive.
    _stream: Option<Box<SendStream>>,
    /// The shared PCM SFX mixer (priority-based, 8 channels).
    /// The cpal callback calls `mix()` on each buffer fill.
    pub mixer: SharedSfxMixer,
    /// The shared MIDI/OPL2 player.  The cpal callback calls
    /// `advance_samples` on each buffer fill to generate FM music output.
    pub midi: SharedMidiPlayer,
}

impl AudioDriver {
    /// Attempt to open the system default audio output at `sample_rate` Hz.
    ///
    /// The cpal callback mixes PCM SFX from [`SfxMixer`] and OPL2 FM audio from
    /// [`MidiPlayer`] into each output buffer frame.
    ///
    /// # Errors
    /// - [`AudioError::NoDevice`] — no default output device.
    /// - [`AudioError::Stream`] — cpal could not create or start the stream.
    ///
    /// # Examples
    /// ```no_run
    /// use doom_audio::driver::AudioDriver;
    ///
    /// // Open the driver at a requested 44.1kHz (though the OS might give us
    /// // something else, which the driver handles gracefully!)
    /// let driver = AudioDriver::open(44100).expect("Failed to open audio device");
    /// ```
    pub fn open(_sample_rate: u32) -> Result<Self, AudioError> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = host.default_output_device().ok_or(AudioError::NoDevice)?;

        let config = device
            .default_output_config()
            .map_err(|e| AudioError::Stream(e.to_string()))?;

        // Use the device's actual sample rate in the callback — not the caller's
        // requested rate — so SFX resampling and OPL timing are correct.
        let actual_rate = config.sample_rate().0;

        eprintln!(
            "[audio-driver] device={:?} channels={} sample_rate={} format={:?}",
            device.name().unwrap_or_default(),
            config.channels(),
            actual_rate,
            config.sample_format(),
        );

        let mixer = Arc::new(Mutex::new(SfxMixer::new()));
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
                    // data is interleaved stereo f32: [L0, R0, L1, R1, ...].
                    let n_mono = (data.len() / 2).max(1);

                    // SfxMixer outputs f32 stereo directly — no i16 conversion needed.
                    let mut sfx_buf = vec![0.0f32; data.len()];
                    let mut opl_buf = vec![0.0f32; n_mono];

                    if let Ok(mut m) = mixer_cb.lock() {
                        m.mix(&mut sfx_buf, actual_rate);
                    }
                    if let Ok(mut mp) = midi_cb.lock() {
                        mp.advance_samples(n_mono, actual_rate, &mut opl_buf);
                    }

                    // Mix SFX (stereo) and OPL (mono) into the output buffer.
                    for i in 0..n_mono {
                        let sfx_l = sfx_buf.get(i * 2).copied().unwrap_or(0.0);
                        let sfx_r = sfx_buf.get(i * 2 + 1).copied().unwrap_or(0.0);
                        let opl = opl_buf.get(i).copied().unwrap_or(0.0) * 0.5;
                        if let Some(out_l) = data.get_mut(i * 2) {
                            *out_l = (sfx_l + opl).clamp(-1.0, 1.0);
                        }
                        if let Some(out_r) = data.get_mut(i * 2 + 1) {
                            *out_r = (sfx_r + opl).clamp(-1.0, 1.0);
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
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_audio::driver::AudioDriver;
    ///
    /// let driver = AudioDriver::null();
    /// ```
    #[must_use]
    pub fn null() -> Self {
        Self {
            _stream: None,
            mixer: Arc::new(Mutex::new(SfxMixer::new())),
            midi: Arc::new(Mutex::new(MidiPlayer::new())),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(all(test, not(feature = "loom")))]
mod tests {
    use super::*;
    use crate::mus::{MusEvent, MusHeader, MusScore};

    #[test]
    fn null_driver_can_be_created() {
        let driver = AudioDriver::null();
        // SfxMixer has 8 channels; just verify the mutex is accessible.
        let mixer = driver.mixer.lock().expect("mutex should not be poisoned");
        assert_eq!(mixer.active_count(), 0);
    }

    #[test]
    fn driver_null_has_midi() {
        let driver = AudioDriver::null();
        // The midi Arc must be valid and the mutex must not be poisoned.
        let mp = driver
            .midi
            .lock()
            .expect("midi mutex should not be poisoned");
        // A freshly created player has no score loaded.
        assert!(
            mp.current_score.is_none(),
            "null driver midi must start with no score"
        );
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
                (
                    0,
                    MusEvent::PlayNote {
                        channel: 0,
                        note: 60,
                        volume: Some(100),
                    },
                ),
                (10, MusEvent::ScoreEnd),
            ],
        };

        // Lock, load, verify — must not panic.
        let mut mp = driver.midi.lock().expect("midi mutex must not be poisoned");
        mp.load_score(score);
        assert!(
            mp.current_score.is_some(),
            "score must be loaded after load_score"
        );
        assert_eq!(mp.event_cursor, 0);
    }
}

#[cfg(all(test, feature = "loom"))]
mod loom_tests {
    use super::*;
    use loom::thread;

    #[test]
    fn havoc_loom_deadlock_test() {
        loom::model(|| {
            let driver = AudioDriver::null();
            let mixer1 = driver.mixer.clone();
            let mixer2 = driver.mixer.clone();

            let t1 = thread::spawn(move || {
                let mut m = mixer1.lock().unwrap();
                m.stop_all();
            });

            let t2 = thread::spawn(move || {
                let mut m = mixer2.lock().unwrap();
                let mut buf = [0.0; 2];
                m.mix(&mut buf, 44100);
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }
}
