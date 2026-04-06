//! PCM sample mixer.
//!
//! Decodes Doom SFX lumps (8-bit unsigned PCM with an 8-byte header) and
//! mixes up to 8 simultaneous voices into a stereo `i16` output buffer.

#[cfg(feature = "loom")]
use loom::sync::Arc;
#[cfg(not(feature = "loom"))]
use std::sync::Arc;

use crate::AudioError;

// ---------------------------------------------------------------------------
// PcmSample
// ---------------------------------------------------------------------------

/// A decoded PCM sound effect sample.
pub struct PcmSample {
    /// Sample rate in Hz as recorded in the SFX lump header.
    pub sample_rate: u32,
    /// Raw 8-bit unsigned PCM data (128 = silence).
    pub data: std::sync::Arc<[u8]>,
}

impl PcmSample {
    /// Parse a raw Doom SFX lump.
    ///
    /// Lump layout:
    /// - bytes 0-1: format tag (u16 LE); must be 3
    /// - bytes 2-3: sample rate (u16 LE)
    /// - bytes 4-7: sample count (u32 LE)
    /// - bytes 8..: raw 8-bit unsigned PCM
    ///
    /// # Errors
    /// Returns [`AudioError::InvalidSfx`] if the data is too short or the
    /// format tag is not 3.
    pub fn parse_sfx_lump(data: &[u8]) -> Result<Self, AudioError> {
        if data.len() < 8 {
            return Err(AudioError::InvalidSfx("lump too short for SFX header"));
        }

        let format = u16::from_le_bytes([data[0], data[1]]);
        if format != 3 {
            return Err(AudioError::InvalidSfx(
                "unexpected SFX format tag (expected 3)",
            ));
        }

        let sample_rate = u32::from(u16::from_le_bytes([data[2], data[3]]));
        let sample_count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;

        let end = 8_usize
            .checked_add(sample_count)
            .ok_or(AudioError::InvalidSfx(
                "lump too short for declared sample count",
            ))?;
        if data.len() < end {
            return Err(AudioError::InvalidSfx(
                "lump too short for declared sample count",
            ));
        }

        Ok(Self {
            sample_rate,
            data: data[8..end].to_vec().into(),
        })
    }

    /// Create a silent sample buffer of `num_samples` frames at `sample_rate`.
    ///
    /// 8-bit unsigned silence is represented by 128.
    #[must_use]
    pub fn silence(sample_rate: u32, num_samples: usize) -> Self {
        Self {
            sample_rate,
            data: vec![128u8; num_samples].into(),
        }
    }
}

// ---------------------------------------------------------------------------
// MixChannel
// ---------------------------------------------------------------------------

/// One slot in the [`Mixer`] voice table.
pub struct MixChannel {
    /// The sample currently assigned to this channel, if any.
    pub sample: Option<Arc<PcmSample>>,
    /// Read position within `sample.data`.
    pub pos: usize,
    /// Volume scale 0–127.
    pub volume: u8,
    /// Whether this channel is currently playing.
    pub active: bool,
}

impl Default for MixChannel {
    fn default() -> Self {
        Self {
            sample: None,
            pos: 0,
            volume: 127,
            active: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Mixer
// ---------------------------------------------------------------------------

/// Eight-voice PCM mixer that outputs interleaved stereo `i16` frames.
pub struct Mixer {
    /// The 8 voice channels.
    pub channels: [MixChannel; 8],
    /// Output sample rate in Hz.
    pub sample_rate: u32,
}

impl Mixer {
    /// Create a new mixer targeting `sample_rate` Hz output.
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            channels: std::array::from_fn(|_| MixChannel::default()),
            sample_rate,
        }
    }

    /// Start (or restart) playback of `sample` on `channel` at the given
    /// `volume` (0–127).
    ///
    /// If `channel >= 8` the call is silently ignored.
    pub fn play(&mut self, channel: usize, sample: Arc<PcmSample>, volume: u8) {
        if channel < 8 {
            self.channels[channel] = MixChannel {
                sample: Some(sample),
                pos: 0,
                volume,
                active: true,
            };
        }
    }

    /// Stop playback on `channel`.
    ///
    /// If `channel >= 8` the call is silently ignored.
    pub fn stop(&mut self, channel: usize) {
        if channel < 8 {
            self.channels[channel].active = false;
        }
    }

    /// Mix all active channels into `out` as interleaved i16 stereo
    /// (L, R, L, R, …).
    ///
    /// `out.len()` should be even (stereo pairs); if it is odd the last
    /// element is left untouched.
    pub fn mix_frame(&mut self, out: &mut [i16]) {
        for frame in out.chunks_mut(2) {
            let mut sum: i32 = 0;

            for ch in &mut self.channels {
                if !ch.active {
                    continue;
                }
                if let Some(ref sample) = ch.sample {
                    let byte = sample.data.get(ch.pos).copied().unwrap_or(128);
                    // Convert 8-bit unsigned to signed, then scale by volume.
                    sum += (byte as i32 - 128) * i32::from(ch.volume) / 127;
                    ch.pos += 1;
                    if ch.pos >= sample.data.len() {
                        ch.active = false;
                    }
                }
            }

            // Scale up to i16 range and clamp.
            let s = (sum * 256).clamp(-32_768, 32_767) as i16;
            if frame.len() == 2 {
                frame[0] = s;
                frame[1] = s;
            }
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
    fn mixer_silent_produces_zeros() {
        let mut mixer = Mixer::new(22_050);
        let mut buf = vec![1i16; 64]; // pre-fill non-zero so we can detect change
        mixer.mix_frame(&mut buf);
        assert!(
            buf.iter().all(|&s| s == 0),
            "silent mixer must output all zeros"
        );
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn mixer_plays_sample() {
        let mut mixer = Mixer::new(22_050);
        let sample = Arc::new(PcmSample {
            sample_rate: 11_025,
            data: vec![255u8; 100].into(),
        });
        mixer.play(0, sample, 127);

        // Mix 50 stereo frames (100 i16 values).
        let mut buf = vec![0i16; 100];
        mixer.mix_frame(&mut buf);

        // Every frame should have a positive (non-zero) value because 255 – 128 = 127 > 0.
        assert_ne!(buf[0], 0, "first sample output should be non-zero");
        assert_ne!(buf[1], 0, "first sample right channel should be non-zero");
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn mixer_channel_stops_at_end() {
        let mut mixer = Mixer::new(22_050);
        let sample = Arc::new(PcmSample {
            sample_rate: 22_050,
            data: vec![200u8; 10].into(),
        });
        mixer.play(0, sample, 127);

        // Mix 20 stereo frames (40 i16 values) — sample is only 10 bytes so
        // it should exhaust after 10 frames.
        let mut buf = vec![0i16; 40];
        mixer.mix_frame(&mut buf);

        assert!(
            !mixer.channels[0].active,
            "channel 0 should be inactive after sample ends"
        );
    }

    #[test]
    fn pcm_parse_valid_lump() {
        let mut lump = Vec::with_capacity(108);
        lump.extend_from_slice(&3u16.to_le_bytes()); // format = 3
        lump.extend_from_slice(&11_025u16.to_le_bytes()); // sample_rate
        lump.extend_from_slice(&100u32.to_le_bytes()); // sample_count
        lump.extend(vec![128u8; 100]); // PCM data

        let sample = PcmSample::parse_sfx_lump(&lump).expect("valid lump should parse");
        assert_eq!(sample.data.len(), 100);
        assert_eq!(sample.sample_rate, 11_025);
    }
}
