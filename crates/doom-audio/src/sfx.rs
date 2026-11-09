//! SFX samples and cache.
//!
//! Provides [`PcmSample`] (a decoded Doom SFX lump) and [`SfxCache`] for
//! storing decoded samples keyed by their Doom SFX ID.  Playback lives in
//! [`crate::sfx_mixer::SfxMixer`].
//!
//! (A previous parallel implementation — an i16 `Mixer` plus a `play_sfx`
//! dispatch helper in a separate `mixer` module — was removed: nothing
//! outside this crate referenced it, it resampled nothing, panned nothing,
//! and stole channel 0 when full, which made it a trap for future edits.)

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

        let end = 8 + sample_count;
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
// SfxCache
// ---------------------------------------------------------------------------

/// A cache of decoded PCM sound effect samples, keyed by Doom SFX ID.
pub struct SfxCache {
    samples: std::collections::HashMap<u16, Arc<PcmSample>>,
}

impl Default for SfxCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SfxCache {
    /// Create an empty [`SfxCache`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            samples: std::collections::HashMap::new(),
        }
    }

    /// Store a decoded PCM sample in the cache under the given SFX `id`.
    ///
    /// If an entry for `id` already exists it is replaced.
    pub fn insert(&mut self, id: u16, sample: Arc<PcmSample>) {
        self.samples.insert(id, sample);
    }

    /// Look up a cached sample by SFX `id`.
    ///
    /// Returns `None` if the ID has not been inserted.
    #[must_use]
    pub fn get(&self, id: u16) -> Option<Arc<PcmSample>> {
        self.samples.get(&id).cloned()
    }

    /// Number of samples currently stored in the cache.
    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Returns `true` if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a trivial, non-silent [`PcmSample`] (a single byte at silence+1).
    fn make_sample() -> Arc<PcmSample> {
        Arc::new(PcmSample {
            sample_rate: 11_025,
            data: vec![200u8; 16].into(),
        })
    }

    #[test]
    #[cfg(not(feature = "loom"))]
    fn sfx_cache_insert_and_get() {
        let mut cache = SfxCache::new();
        cache.insert(42, make_sample());
        assert!(
            cache.get(42).is_some(),
            "get(42) should return Some after insert"
        );
    }

    #[test]
    fn sfx_cache_miss_returns_none() {
        let cache = SfxCache::new();
        assert!(
            cache.get(99).is_none(),
            "get on empty cache should return None"
        );
    }

    #[test]
    fn pcm_sample_parse_roundtrip() {
        // tag=3, rate=22050, count=4, payload bytes.
        let mut lump = vec![3u8, 0, 0x22, 0x56, 4, 0, 0, 0];
        lump.extend_from_slice(&[10, 20, 30, 40]);
        let sample = PcmSample::parse_sfx_lump(&lump).expect("valid lump");
        assert_eq!(sample.sample_rate, 22_050);
        assert_eq!(&*sample.data, &[10, 20, 30, 40]);
    }

    #[test]
    fn pcm_sample_parse_rejects_bad_tag() {
        let lump = vec![1u8, 0, 0x22, 0x56, 0, 0, 0, 0];
        assert!(PcmSample::parse_sfx_lump(&lump).is_err());
    }
}
