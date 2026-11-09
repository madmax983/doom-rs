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
    /// ## Examples
    /// ```rust,no_run
    /// use doom_audio::driver::AudioDriver;
    ///
    /// // Open the driver at a requested 44.1kHz (though the OS might give us
    /// // something else, which the driver handles gracefully!)
    /// let driver = AudioDriver::open(44100).expect("Failed to open audio device");
    /// ```
    pub fn open(sample_rate: u32) -> Result<Self, AudioError> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = host.default_output_device().ok_or(AudioError::NoDevice)?;

        // Pick a supported config: prefer the caller's requested rate when
        // the device offers it, else the device default. (The requested
        // rate used to be silently ignored.)
        let supported = pick_supported_config(&device, sample_rate)?;
        // Use the config's actual sample rate in the callback so SFX
        // resampling and OPL timing are correct.
        let actual_rate = supported.sample_rate().0;
        let channels = usize::from(supported.channels()).max(1);
        let format = supported.sample_format();

        log::debug!(
            "[audio-driver] device={:?} channels={} sample_rate={} format={:?}",
            device.name().unwrap_or_default(),
            channels,
            actual_rate,
            format,
        );

        let mixer = Arc::new(Mutex::new(SfxMixer::new()));
        let midi = Arc::new(Mutex::new(MidiPlayer::new()));

        // Request a small, explicit buffer (512 frames ≈ 10 ms at 48 kHz)
        // for tight SFX latency; fall back to the device default size if
        // the backend rejects it.
        let mut config: cpal::StreamConfig = supported.into();
        config.buffer_size = cpal::BufferSize::Fixed(512);

        let stream = build_stream_for_format(
            &device,
            &config,
            channels,
            actual_rate,
            format,
            Arc::clone(&mixer),
            Arc::clone(&midi),
        )
        .or_else(|_| {
            let mut fallback = config.clone();
            fallback.buffer_size = cpal::BufferSize::Default;
            build_stream_for_format(
                &device,
                &fallback,
                channels,
                actual_rate,
                format,
                Arc::clone(&mixer),
                Arc::clone(&midi),
            )
        })?;

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
    /// ## Examples
    ///
    /// ```
    /// use doom_audio::driver::AudioDriver;
    ///
    /// # #[cfg(not(feature = "loom"))]
    /// # {
    /// let driver = AudioDriver::null();
    /// # }
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
// Stream construction helpers
// ---------------------------------------------------------------------------

/// Choose a supported output config for `device`, preferring one that
/// offers the requested sample rate; otherwise the device default.
fn pick_supported_config(
    device: &cpal::Device,
    requested_rate: u32,
) -> Result<cpal::SupportedStreamConfig, AudioError> {
    use cpal::traits::DeviceTrait;

    if requested_rate > 0 {
        if let Ok(configs) = device.supported_output_configs() {
            for range in configs {
                let rate = cpal::SampleRate(requested_rate);
                if range.min_sample_rate() <= rate && rate <= range.max_sample_rate() {
                    return Ok(range.with_sample_rate(rate));
                }
            }
        }
    }
    device
        .default_output_config()
        .map_err(|e| AudioError::Stream(e.to_string()))
}

/// Build and start an output stream in the device's sample format.
#[allow(clippy::too_many_arguments)]
fn build_stream_for_format(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    sample_rate: u32,
    format: cpal::SampleFormat,
    mixer: SharedSfxMixer,
    midi: SharedMidiPlayer,
) -> Result<cpal::Stream, AudioError> {
    match format {
        cpal::SampleFormat::F32 => {
            build_stream::<f32>(device, config, channels, sample_rate, mixer, midi, |s| s)
        }
        cpal::SampleFormat::I16 => build_stream::<i16>(
            device,
            config,
            channels,
            sample_rate,
            mixer,
            midi,
            |s| (s.clamp(-1.0, 1.0) * 32_767.0) as i16,
        ),
        cpal::SampleFormat::U16 => build_stream::<u16>(
            device,
            config,
            channels,
            sample_rate,
            mixer,
            midi,
            |s| ((s.clamp(-1.0, 1.0) * 0.5 + 0.5) * 65_535.0) as u16,
        ),
        _ => Err(AudioError::Stream(format!(
            "unsupported sample format: {format:?}"
        ))),
    }
}

/// Build an output stream for sample type `T`.
///
/// The realtime callback is allocation-free in steady state (scratch
/// buffers are preallocated outside it and only grow if the backend hands
/// over a larger buffer than requested) and never blocks: it `try_lock`s
/// the mixer and sequencer, rendering silence for whichever is contended
/// rather than stalling the audio thread on a mutex.
fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    sample_rate: u32,
    mixer: SharedSfxMixer,
    midi: SharedMidiPlayer,
    convert: fn(f32) -> T,
) -> Result<cpal::Stream, AudioError>
where
    T: cpal::SizedSample + Send + 'static,
{
    use cpal::traits::DeviceTrait;

    let channels = channels.max(1);
    // Preallocate scratch for the requested buffer size (512 frames) with
    // headroom; resized only if the backend delivers something larger.
    let mut sfx_buf: Vec<f32> = vec![0.0; 1024 * 2];
    let mut opl_buf: Vec<f32> = vec![0.0; 1024];

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [T], _info: &cpal::OutputCallbackInfo| {
                let frames = data.len() / channels;
                if frames == 0 {
                    return;
                }
                if sfx_buf.len() < frames * 2 {
                    sfx_buf.resize(frames * 2, 0.0);
                }
                if opl_buf.len() < frames {
                    opl_buf.resize(frames, 0.0);
                }
                let sfx = &mut sfx_buf[..frames * 2];
                let opl = &mut opl_buf[..frames];
                sfx.fill(0.0);
                opl.fill(0.0);

                if let Ok(mut m) = mixer.try_lock() {
                    m.mix(sfx, sample_rate);
                }
                if let Ok(mut mp) = midi.try_lock() {
                    // Music gain is owned by the player (config-driven).
                    mp.advance_samples(frames, sample_rate, opl);
                }

                for frame in 0..frames {
                    let sfx_l = sfx[frame * 2];
                    let sfx_r = sfx[frame * 2 + 1];
                    let music = opl[frame];
                    for (c, out) in data[frame * channels..(frame + 1) * channels]
                        .iter_mut()
                        .enumerate()
                    {
                        let v = match (channels, c) {
                            (1, _) => (sfx_l + sfx_r) * 0.5 + music,
                            (_, 0) => sfx_l + music,
                            (_, 1) => sfx_r + music,
                            _ => 0.0,
                        };
                        *out = convert(v.clamp(-1.0, 1.0));
                    }
                }
            },
            |err| log::error!("audio stream error: {err}"),
            None,
        )
        .map_err(|e| AudioError::Stream(e.to_string()))?;
    Ok(stream)
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
                let mut m = mixer1.lock().expect("value must exist in test");
                m.stop_all();
            });

            let t2 = thread::spawn(move || {
                let mut m = mixer2.lock().expect("value must exist in test");
                let mut buf = [0.0; 2];
                m.mix(&mut buf, 44100);
            });

            t1.join().expect("value must exist in test");
            t2.join().expect("value must exist in test");
        });
    }
}
