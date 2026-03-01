//! cpal audio output driver.
//!
//! Wraps a `cpal` output stream and routes its callback through a shared
//! [`Mixer`].  For environments without a real audio device (CI, tests) use
//! [`AudioDriver::null`] which bypasses cpal entirely.

use std::sync::{Arc, Mutex};

use crate::{mixer::Mixer, AudioError};

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
    /// The shared mixer.  Callers queue sounds here; the cpal callback drains
    /// it on each audio buffer fill.
    pub mixer: Arc<Mutex<Mixer>>,
}

impl AudioDriver {
    /// Attempt to open the system default audio output at `sample_rate` Hz.
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
        let mixer_cb = Arc::clone(&mixer);

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    let mut buf = vec![0i16; data.len()];
                    if let Ok(mut m) = mixer_cb.lock() {
                        m.mix_frame(&mut buf);
                    }
                    for (out, s) in data.iter_mut().zip(buf.iter()) {
                        *out = f32::from(*s) / 32_768.0;
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
        })
    }

    /// Create a driver that owns no real stream — safe for headless / test use.
    #[must_use]
    pub fn null() -> Self {
        Self {
            _stream: None,
            mixer: Arc::new(Mutex::new(Mixer::new(44_100))),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_driver_can_be_created() {
        let driver = AudioDriver::null();
        let mixer = driver.mixer.lock().expect("mutex should not be poisoned");
        assert_eq!(mixer.sample_rate, 44_100);
    }
}
