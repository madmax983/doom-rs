//! SFX dispatch — caching and playback of PCM sound effects.
//!
//! Provides [`SfxCache`] for storing decoded [`PcmSample`] lumps keyed by
//! their Doom SFX ID, and [`play_sfx`] for dispatching a sound effect onto
//! the nearest available [`Mixer`] channel.

use std::sync::Arc;

use crate::mixer::{Mixer, PcmSample};

// ---------------------------------------------------------------------------
// SfxPriority
// ---------------------------------------------------------------------------

/// Priority level for a sound effect (higher value = more important).
///
/// Used by Doom to decide which active sound to evict when all mixer channels
/// are busy and a new, higher-priority sound needs to play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SfxPriority(pub u8);

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
// play_sfx
// ---------------------------------------------------------------------------

/// Dispatch sound effect `sfx_id` onto the [`Mixer`] at the given `volume`
/// (0–127).
///
/// Channel selection:
/// 1. The first inactive mixer channel is used.
/// 2. If all 8 channels are busy, channel 0 is stolen (lowest-priority
///    override — intentional fallback).
///
/// If `sfx_id` is not present in `cache` the call is a no-op (no panic).
pub fn play_sfx(mixer: &mut Mixer, cache: &SfxCache, sfx_id: u16, volume: u8) {
    let Some(sample) = cache.get(sfx_id) else {
        return;
    };

    // Find the first inactive channel; fall back to channel 0 if all are busy.
    // SAFETY(unwrap): the iterator range is 0..8 and `unwrap_or(0)` guarantees
    // the result is always a valid channel index.
    let ch = (0..8usize)
        .find(|&i| !mixer.channels[i].active)
        .unwrap_or(0); // steal ch 0 when all channels are busy

    mixer.play(ch, sample, volume);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixer::Mixer;

    /// Build a trivial, non-silent [`PcmSample`] (a single byte at silence+1).
    fn make_sample() -> Arc<PcmSample> {
        Arc::new(PcmSample {
            sample_rate: 11_025,
            data: vec![200u8; 16].into(),
        })
    }

    #[test]
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
    fn play_sfx_starts_channel() {
        let mut mixer = Mixer::new(22_050);
        let mut cache = SfxCache::new();
        cache.insert(1, make_sample());

        play_sfx(&mut mixer, &cache, 1, 100);

        assert!(
            mixer.channels[0].active,
            "channel 0 should be active after play_sfx"
        );
    }

    #[test]
    fn play_sfx_unknown_id_no_panic() {
        let mut mixer = Mixer::new(22_050);
        let cache = SfxCache::new();

        // Must not panic; all channels remain inactive.
        play_sfx(&mut mixer, &cache, 0xDEAD, 64);

        for i in 0..8 {
            assert!(
                !mixer.channels[i].active,
                "channel {i} should remain inactive when sfx_id is not in cache"
            );
        }
    }
}
