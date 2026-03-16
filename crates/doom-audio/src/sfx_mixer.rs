//! Priority-based SFX channel mixer with stereo panning.
//!
//! Welcome to the heart of the engine's auditory experience! This module
//! provides the [`SfxMixer`], which manages up to [`MAX_CHANNELS`] simultaneous
//! sound effects. It ensures that when the chaotic symphony of a demon horde
//! overwhelms the hardware, the most crucial sounds (like your own weapon firing!)
//! are never silenced.
//!
//! It achieves this through priority-based channel allocation, per-channel volume
//! and stereo panning, and 8-bit unsigned PCM to stereo f32 mixing.

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of simultaneous sound channels.
pub const MAX_CHANNELS: usize = 8;

// ---------------------------------------------------------------------------
// SfxPriority
// ---------------------------------------------------------------------------

/// Priority levels for sound effects.
///
/// Higher-priority sounds can steal channels from lower-priority ones when
/// all channels are occupied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SfxPriority {
    /// Ambient/background sounds.
    Low = 0,
    /// Standard gameplay sounds (doors, pickups).
    Medium = 1,
    /// Important gameplay sounds (enemies, damage).
    High = 2,
    /// Player weapon sounds (highest priority).
    Weapon = 3,
}

// ---------------------------------------------------------------------------
// SfxChannel
// ---------------------------------------------------------------------------

/// Source sample rate for Doom PCM sound effects.
pub const SFX_SOURCE_RATE: u32 = 11_025;

/// A single sound channel playing a sound effect.
#[derive(Clone, Debug)]
pub struct SfxChannel {
    /// Sound effect identifier (e.g., SFX lump index).
    pub sfx_id: u16,
    /// Current playback position as a fixed-point 16.16 fraction into `data`.
    /// The integer part is the source sample index; the fractional part allows
    /// sub-sample stepping for arbitrary output/source rate ratios.
    pub position_fp: u64,
    /// Total number of source samples.
    pub length: usize,
    /// Volume (`0.0` to `1.0`).
    pub volume: f32,
    /// Stereo panning (`-1.0` to `1.0`).
    pub pan: f32,
    /// Priority for channel allocation.
    pub priority: SfxPriority,
    /// Whether this channel is currently playing.
    pub active: bool,
    /// Raw PCM data (8-bit unsigned, [`SFX_SOURCE_RATE`] Hz).
    pub data: Vec<u8>,
}

// ---------------------------------------------------------------------------
// SfxMixer
// ---------------------------------------------------------------------------

/// Priority-based SFX mixer managing up to [`MAX_CHANNELS`] simultaneous sounds.
///
/// # Channel allocation
///
/// When a new sound is requested and all channels are occupied:
/// 1. Find the channel with the lowest priority.
/// 2. If the new sound has higher or equal priority, steal that channel.
/// 3. If all channels have strictly higher priority, the new sound is dropped.
#[derive(Clone, Debug)]
pub struct SfxMixer {
    channels: [Option<SfxChannel>; MAX_CHANNELS],
}

impl Default for SfxMixer {
    fn default() -> Self {
        Self::new()
    }
}

impl SfxMixer {
    fn make_channel(
        sfx_id: u16,
        data: Vec<u8>,
        volume: f32,
        pan: f32,
        priority: SfxPriority,
    ) -> SfxChannel {
        let length = data.len();

        SfxChannel {
            sfx_id,
            position_fp: 0,
            length,
            volume,
            pan,
            priority,
            active: true,
            data,
        }
    }

    /// Create a new mixer with all channels empty.
    #[must_use]
    pub fn new() -> Self {
        Self {
            channels: std::array::from_fn(|_| None),
        }
    }

    /// Start playing a sound effect. Returns the channel index, or `None`
    /// if all channels are occupied by higher-priority sounds.
    ///
    /// # Channel selection
    ///
    /// 1. First empty slot is used.
    /// 2. First inactive channel is reused.
    /// 3. If all channels are active, the lowest-priority channel is stolen
    ///    (provided the new sound has >= that priority).
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_audio::{SfxMixer, SfxPriority};
    ///
    /// let mut mixer = SfxMixer::new();
    /// let sfx_data = vec![128; 1024]; // dummy silent PCM data
    ///
    /// // Play a high-priority weapon sound
    /// let channel = mixer.play(1, sfx_data, 1.0, 0.0, SfxPriority::Weapon);
    /// assert!(channel.is_some());
    /// ```
    pub fn play(
        &mut self,
        sfx_id: u16,
        data: Vec<u8>,
        volume: f32,
        pan: f32,
        priority: SfxPriority,
    ) -> Option<usize> {
        let channel = Self::make_channel(sfx_id, data, volume, pan, priority);

        // 1. Find first empty (None) slot.
        if let Some(idx) = self.channels.iter().position(Option::is_none) {
            self.channels[idx] = Some(channel);
            return Some(idx);
        }

        // 2. Find first inactive channel.
        if let Some(idx) = self
            .channels
            .iter()
            .position(|ch| ch.as_ref().is_some_and(|c| !c.active))
        {
            self.channels[idx] = Some(channel);
            return Some(idx);
        }

        // 3. Steal the lowest-priority active channel if new sound has >= priority.
        let (lowest_idx, lowest_pri) = self
            .channels
            .iter()
            .enumerate()
            .filter_map(|(i, ch)| ch.as_ref().map(|c| (i, c.priority)))
            .min_by_key(|&(_, pri)| pri)?;

        if priority >= lowest_pri {
            self.channels[lowest_idx] = Some(channel);
            Some(lowest_idx)
        } else {
            None
        }
    }

    /// Replace a specific channel with a newly started sound effect.
    ///
    /// This preserves Doom's "one live channel per origin" rule: if the same
    /// actor starts another sound (like a chaingun rapidly firing), the old one
    /// is restarted in place instead of allocating a second channel, preventing
    /// audio clutter.
    ///
    /// # Panics
    ///
    /// Panics if `channel` is greater than or equal to [`MAX_CHANNELS`] in debug builds.
    /// Callers must ensure they only pass indices returned by [`SfxMixer::play`].
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_audio::{SfxMixer, SfxPriority};
    ///
    /// let mut mixer = SfxMixer::new();
    /// let sfx_data = vec![128; 1024]; // dummy silent PCM data
    ///
    /// // Play a sound, which gives us a channel index.
    /// if let Some(channel) = mixer.play(42, sfx_data.clone(), 1.0, 0.0, SfxPriority::Weapon) {
    ///     // The same entity makes another sound immediately. We restart it on the same channel!
    ///     mixer.play_on_channel(channel, 43, sfx_data, 1.0, 0.0, SfxPriority::Weapon);
    /// }
    /// ```
    pub fn play_on_channel(
        &mut self,
        channel: usize,
        sfx_id: u16,
        data: Vec<u8>,
        volume: f32,
        pan: f32,
        priority: SfxPriority,
    ) -> usize {
        debug_assert!(
            channel < MAX_CHANNELS,
            "channel index {channel} out of range"
        );
        self.channels[channel] = Some(Self::make_channel(sfx_id, data, volume, pan, priority));
        channel
    }

    /// Update the spatial parameters of a specific channel.
    ///
    /// Called each tic with updated listener position to keep sounds
    /// spatially accurate as the listener moves.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_audio::{SfxMixer, SfxPriority};
    ///
    /// let mut mixer = SfxMixer::new();
    /// let sfx_data = vec![128; 1024];
    ///
    /// if let Some(channel) = mixer.play(42, sfx_data, 1.0, 0.0, SfxPriority::Medium) {
    ///     // Move the sound to the far left speaker
    ///     mixer.update_spatial(channel, 1.0, -1.0);
    /// }
    /// ```
    pub fn update_spatial(&mut self, channel: usize, volume: f32, pan: f32) {
        if channel < MAX_CHANNELS {
            if let Some(ch) = &mut self.channels[channel] {
                if ch.active {
                    ch.volume = volume;
                    ch.pan = pan;
                }
            }
        }
    }

    /// Mix all active channels into a stereo output buffer.
    ///
    /// Output is interleaved stereo f32 samples (`[L0, R0, L1, R1, ...]`)
    /// at `output_rate` Hz.  Source PCM is assumed to be [`SFX_SOURCE_RATE`] Hz
    /// (11025 Hz); nearest-neighbour resampling upsamples to the output rate so
    /// sounds play at the correct pitch and duration regardless of device rate.
    ///
    /// After mixing, channels whose data is exhausted are marked inactive.
    ///
    /// # Examples
    ///
    /// ```
    /// use doom_audio::{SfxMixer, SfxPriority};
    ///
    /// let mut mixer = SfxMixer::new();
    /// mixer.play(42, vec![200; 1024], 1.0, 0.0, SfxPriority::Weapon);
    ///
    /// // Interleaved stereo f32 buffer
    /// let mut output = vec![0.0; 256];
    ///
    /// // Mix the sound into the output buffer at 44.1kHz
    /// mixer.mix(&mut output, 44100);
    /// ```
    #[allow(clippy::cast_precision_loss)]
    pub fn mix(&mut self, output: &mut [f32], output_rate: u32) {
        // Fixed-point 16.16 step: how many source samples to advance per output frame.
        // e.g. at 48000 Hz output, 11025 Hz source: step = 11025/48000 * 65536 ≈ 15052
        let step_fp = ((SFX_SOURCE_RATE as u64) << 16) / (output_rate as u64).max(1);

        // Clear output buffer.
        output.fill(0.0);

        for ch in self.channels.iter_mut().flatten() {
            if !ch.active {
                continue;
            }

            // Pan gains are constant for this channel across the whole buffer.
            // pan = -1.0 => left_gain = 1.0, right_gain = 0.0
            // pan =  0.0 => left_gain = 0.5, right_gain = 0.5
            // pan =  1.0 => left_gain = 0.0, right_gain = 1.0
            let left_gain = (1.0 - ch.pan) * 0.5;
            let right_gain = (1.0 + ch.pan) * 0.5;

            // Process stereo frame pairs.
            for i in (0..output.len()).step_by(2) {
                let src_idx = (ch.position_fp >> 16) as usize;
                if src_idx >= ch.length {
                    ch.active = false;
                    break;
                }

                // Convert 8-bit unsigned PCM to f32 in [-1.0, 1.0].
                let sample = (ch.data[src_idx] as f32 - 128.0) / 128.0;
                let scaled = sample * ch.volume;

                output[i] += scaled * left_gain;
                if i + 1 < output.len() {
                    output[i + 1] += scaled * right_gain;
                }

                ch.position_fp += step_fp;
            }
        }

        // Clamp output to [-1.0, 1.0] to prevent clipping.
        for sample in output.iter_mut() {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }

    /// Stop all sounds immediately.
    pub fn stop_all(&mut self) {
        for slot in &mut self.channels {
            *slot = None;
        }
    }

    /// Number of currently active channels.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.channels
            .iter()
            .filter(|ch| ch.as_ref().is_some_and(|c| c.active))
            .count()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixer_new_has_no_active_channels() {
        let mixer = SfxMixer::new();
        assert_eq!(mixer.active_count(), 0);
    }

    #[test]
    fn mixer_play_activates_channel() {
        let mut mixer = SfxMixer::new();
        let data = vec![128u8; 100]; // silence
        let ch = mixer.play(1, data, 1.0, 0.0, SfxPriority::Medium);
        assert!(ch.is_some());
        assert_eq!(mixer.active_count(), 1);
    }

    #[test]
    fn mixer_max_channels_respected() {
        let mut mixer = SfxMixer::new();
        for i in 0..MAX_CHANNELS {
            let data = vec![128u8; 1000];
            assert!(
                mixer
                    .play(i as u16, data, 1.0, 0.0, SfxPriority::Medium)
                    .is_some()
            );
        }
        assert_eq!(mixer.active_count(), MAX_CHANNELS);
    }

    #[test]
    fn mixer_priority_steals_low_priority_channel() {
        let mut mixer = SfxMixer::new();
        // Fill all channels with low priority.
        for i in 0..MAX_CHANNELS {
            let data = vec![128u8; 1000];
            mixer.play(i as u16, data, 1.0, 0.0, SfxPriority::Low);
        }
        // High priority should steal a channel.
        let data = vec![128u8; 100];
        let ch = mixer.play(99, data, 1.0, 0.0, SfxPriority::High);
        assert!(ch.is_some());
    }

    #[test]
    fn mixer_cannot_steal_higher_priority() {
        let mut mixer = SfxMixer::new();
        // Fill all channels with Weapon (highest) priority.
        for i in 0..MAX_CHANNELS {
            let data = vec![128u8; 1000];
            mixer.play(i as u16, data, 1.0, 0.0, SfxPriority::Weapon);
        }
        // Low priority should NOT steal.
        let data = vec![128u8; 100];
        let ch = mixer.play(99, data, 1.0, 0.0, SfxPriority::Low);
        assert!(ch.is_none(), "low priority should not steal from weapon");
    }

    #[test]
    fn mixer_mix_produces_output() {
        let mut mixer = SfxMixer::new();
        // Play a non-silent sound (200 > 128 = positive signal).
        let data = vec![200u8; 100];
        mixer.play(1, data, 1.0, 0.0, SfxPriority::Medium);
        let mut output = vec![0.0f32; 200]; // 100 stereo samples
        mixer.mix(&mut output, 11025);
        // At least some samples should be non-zero.
        assert!(output.iter().any(|&s| s.abs() > 0.01));
    }

    #[test]
    fn mixer_stop_all_clears_channels() {
        let mut mixer = SfxMixer::new();
        let data = vec![128u8; 100];
        mixer.play(1, data, 1.0, 0.0, SfxPriority::Medium);
        mixer.stop_all();
        assert_eq!(mixer.active_count(), 0);
    }

    #[test]
    fn mixer_mix_applies_panning() {
        let mut mixer = SfxMixer::new();
        let data = vec![200u8; 100];
        mixer.play(1, data, 1.0, 1.0, SfxPriority::Medium); // full right
        let mut output = vec![0.0f32; 200];
        mixer.mix(&mut output, 11025);
        // Left channel (even indices) should be near-silent.
        // Right channel (odd indices) should have signal.
        let left_energy: f32 = output.iter().step_by(2).map(|s| s * s).sum();
        let right_energy: f32 = output.iter().skip(1).step_by(2).map(|s| s * s).sum();
        assert!(
            right_energy > left_energy * 5.0,
            "Right should be much louder than left (right={right_energy}, left={left_energy})"
        );
    }

    #[test]
    fn mixer_mix_applies_left_panning() {
        let mut mixer = SfxMixer::new();
        let data = vec![200u8; 100];
        mixer.play(1, data, 1.0, -1.0, SfxPriority::Medium); // full left
        let mut output = vec![0.0f32; 200];
        mixer.mix(&mut output, 11025);
        let left_energy: f32 = output.iter().step_by(2).map(|s| s * s).sum();
        let right_energy: f32 = output.iter().skip(1).step_by(2).map(|s| s * s).sum();
        assert!(
            left_energy > right_energy * 5.0,
            "Left should be much louder than right (left={left_energy}, right={right_energy})"
        );
    }

    #[test]
    fn mixer_channel_finishes_when_data_exhausted() {
        let mut mixer = SfxMixer::new();
        let data = vec![128u8; 10]; // very short sound
        mixer.play(1, data, 1.0, 0.0, SfxPriority::Medium);
        let mut output = vec![0.0f32; 200]; // more than enough
        mixer.mix(&mut output, 11025);
        assert_eq!(
            mixer.active_count(),
            0,
            "sound should have finished after data exhausted"
        );
    }

    #[test]
    fn mixer_update_spatial_changes_params() {
        let mut mixer = SfxMixer::new();
        let data = vec![200u8; 1000];
        let ch = mixer
            .play(1, data, 1.0, 0.0, SfxPriority::Medium)
            .expect("should get channel");

        // Update spatial params.
        mixer.update_spatial(ch, 0.5, -0.8);

        // Verify the channel was updated.
        let channel = mixer.channels[ch].as_ref().expect("channel should exist");
        assert!((channel.volume - 0.5).abs() < 0.01);
        assert!((channel.pan - (-0.8)).abs() < 0.01);
    }

    #[test]
    fn mixer_play_on_channel_restarts_in_place() {
        let mut mixer = SfxMixer::new();
        let ch = mixer
            .play(1, vec![200u8; 1000], 1.0, 0.0, SfxPriority::Medium)
            .expect("should allocate a channel");

        let replaced =
            mixer.play_on_channel(ch, 2, vec![220u8; 1000], 0.4, -0.5, SfxPriority::High);

        assert_eq!(replaced, ch);
        assert_eq!(mixer.active_count(), 1);

        let channel = mixer.channels[ch].as_ref().expect("channel should exist");
        assert_eq!(channel.sfx_id, 2);
        assert_eq!(channel.priority, SfxPriority::High);
        assert!((channel.volume - 0.4).abs() < 0.01);
        assert!((channel.pan - (-0.5)).abs() < 0.01);
        assert_eq!(channel.position_fp, 0);
    }

    #[test]
    fn mixer_silent_output_when_no_sounds() {
        let mut mixer = SfxMixer::new();
        let mut output = vec![1.0f32; 100]; // pre-fill with non-zero
        mixer.mix(&mut output, 11025);
        assert!(
            output.iter().all(|&s| s == 0.0),
            "silent mixer must produce all zeros"
        );
    }

    #[test]
    fn mixer_reuses_inactive_channel() {
        let mut mixer = SfxMixer::new();
        // Play a short sound that will finish quickly.
        let data = vec![200u8; 5];
        mixer.play(1, data, 1.0, 0.0, SfxPriority::Medium);

        // Exhaust it.
        let mut output = vec![0.0f32; 100];
        mixer.mix(&mut output, 11025);
        assert_eq!(mixer.active_count(), 0);

        // Now play another sound -- should reuse the inactive channel.
        let data2 = vec![200u8; 100];
        let ch = mixer.play(2, data2, 1.0, 0.0, SfxPriority::Low);
        assert!(ch.is_some());
        assert_eq!(mixer.active_count(), 1);
    }

    #[test]
    fn mixer_equal_priority_can_steal() {
        let mut mixer = SfxMixer::new();
        // Fill all channels with Medium priority.
        for i in 0..MAX_CHANNELS {
            let data = vec![128u8; 1000];
            mixer.play(i as u16, data, 1.0, 0.0, SfxPriority::Medium);
        }
        // Another Medium priority should be able to steal.
        let data = vec![128u8; 100];
        let ch = mixer.play(99, data, 1.0, 0.0, SfxPriority::Medium);
        assert!(
            ch.is_some(),
            "equal priority should steal lowest-priority channel"
        );
    }

    #[test]
    fn mixer_volume_zero_produces_silence() {
        let mut mixer = SfxMixer::new();
        let data = vec![255u8; 100]; // loud sound
        mixer.play(1, data, 0.0, 0.0, SfxPriority::Medium); // zero volume
        let mut output = vec![0.0f32; 200];
        mixer.mix(&mut output, 11025);
        assert!(
            output.iter().all(|&s| s.abs() < 0.001),
            "zero volume should produce silence"
        );
    }

    // -----------------------------------------------------------------------
    // Regression tests: the specific bug where play_sfx blindly stole channel
    // 0 (unwrap_or(0)) instead of using priority, causing weapon sounds to be
    // silenced by a flood of lower-priority monster sounds.
    // -----------------------------------------------------------------------

    /// Weapon sound must steal a channel even when all 8 are full of monster
    /// attack sounds (Medium priority).  This was the original failure mode.
    #[test]
    fn weapon_sound_survives_monster_attack_flood() {
        let mut mixer = SfxMixer::new();
        for i in 0..MAX_CHANNELS {
            mixer.play(i as u16, vec![128u8; 1000], 1.0, 0.0, SfxPriority::Medium);
        }
        assert_eq!(mixer.active_count(), MAX_CHANNELS);

        let ch = mixer.play(99, vec![128u8; 100], 1.0, 0.0, SfxPriority::Weapon);
        assert!(
            ch.is_some(),
            "weapon sound must steal a channel from Medium-priority monster sounds"
        );
    }

    /// Weapon sound must steal a channel even when all 8 are full of high-priority
    /// monster wake/die sounds.
    #[test]
    fn weapon_sound_survives_monster_wake_flood() {
        let mut mixer = SfxMixer::new();
        for i in 0..MAX_CHANNELS {
            mixer.play(i as u16, vec![128u8; 1000], 1.0, 0.0, SfxPriority::High);
        }
        assert_eq!(mixer.active_count(), MAX_CHANNELS);

        let ch = mixer.play(99, vec![128u8; 100], 1.0, 0.0, SfxPriority::Weapon);
        assert!(
            ch.is_some(),
            "weapon sound must steal a channel from High-priority monster sounds"
        );
    }

    /// Monster sounds (Medium) must NOT steal channels that are held by weapon
    /// sounds (Weapon priority).
    #[test]
    fn monster_attack_cannot_steal_weapon_channel() {
        let mut mixer = SfxMixer::new();
        for i in 0..MAX_CHANNELS {
            mixer.play(i as u16, vec![128u8; 1000], 1.0, 0.0, SfxPriority::Weapon);
        }
        assert_eq!(mixer.active_count(), MAX_CHANNELS);

        let ch = mixer.play(99, vec![128u8; 100], 1.0, 0.0, SfxPriority::Medium);
        assert!(
            ch.is_none(),
            "monster attack sound must not steal a Weapon-priority channel"
        );
    }

    /// Monster wake/die sounds (High) must NOT steal weapon channels.
    #[test]
    fn monster_wake_cannot_steal_weapon_channel() {
        let mut mixer = SfxMixer::new();
        for i in 0..MAX_CHANNELS {
            mixer.play(i as u16, vec![128u8; 1000], 1.0, 0.0, SfxPriority::Weapon);
        }

        let ch = mixer.play(99, vec![128u8; 100], 1.0, 0.0, SfxPriority::High);
        assert!(
            ch.is_none(),
            "monster wake/die sound must not steal a Weapon-priority channel"
        );
    }

    #[test]
    fn mixer_clamps_output() {
        let mut mixer = SfxMixer::new();
        // Play multiple loud sounds on different channels to try to exceed [-1, 1].
        for i in 0..MAX_CHANNELS {
            let data = vec![255u8; 100]; // max positive signal
            mixer.play(i as u16, data, 1.0, 0.0, SfxPriority::Medium);
        }
        let mut output = vec![0.0f32; 200];
        mixer.mix(&mut output, 11025);
        assert!(
            output.iter().all(|&s| (-1.0..=1.0).contains(&s)),
            "output should be clamped to [-1.0, 1.0]"
        );
    }
}
