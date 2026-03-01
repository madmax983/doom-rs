//! MUS→OPL2 sequencer.
//!
//! Drives an [`OplChip`] from a decoded [`MusScore`].  Maps MUS channels to
//! OPL2 hardware channels and translates [`MusEvent`] values into OPL2
//! register writes.

use crate::{
    mus::{MusEvent, MusScore},
    opl::OplChip,
};

// ---------------------------------------------------------------------------
// OPL2 register-slot helpers
// ---------------------------------------------------------------------------

/// Return the modulator register offset for OPL2 channel `ch` (0–8).
///
/// OPL2 modulator register offsets per channel:
/// ch:  0   1   2   3   4   5   6   7   8
/// mod: 0   1   2   8   9  10  16  17  18
fn opl2_mod_reg(ch: u8) -> u8 {
    const MOD: [u8; 9] = [0, 1, 2, 8, 9, 10, 16, 17, 18];
    MOD[(ch as usize) % 9]
}

/// Return the carrier register offset for OPL2 channel `ch` (0–8).
///
/// OPL2 carrier register offsets per channel:
/// ch:  0   1   2   3   4   5   6   7   8
/// car: 3   4   5  11  12  13  19  20  21
fn opl2_car_reg(ch: u8) -> u8 {
    const CAR: [u8; 9] = [3, 4, 5, 11, 12, 13, 19, 20, 21];
    CAR[(ch as usize) % 9]
}

// ---------------------------------------------------------------------------
// OPL2 note→frequency translation
// ---------------------------------------------------------------------------

/// Translate a MIDI note number (0–127) to an OPL2 (block, F-number) pair.
///
/// Formula: `freq_hz = 440 * 2^((note - 69) / 12)`
///          `fnum   = freq_hz * 2^(20 - block) / 49716`
///
/// The base F-numbers below are computed at block 4, with a 2× upward shift
/// per octave encoded in the block field.  Block is clamped to 0–7.
fn note_to_block_fnum(note: u8) -> (u8, u16) {
    // One entry per chromatic semitone within an octave, at block 4.
    // These match the standard Doom/OPL2 note table.
    const BASE_FNUM: [u16; 12] = [172, 183, 194, 205, 217, 230, 244, 258, 274, 290, 307, 326];
    let octave = (note / 12).saturating_sub(1);
    let semitone = note % 12;
    let block = octave.min(7);
    let fnum = BASE_FNUM[semitone as usize];
    (block, fnum)
}

// ---------------------------------------------------------------------------
// Instrument loading
// ---------------------------------------------------------------------------

/// Write a default sine-wave instrument definition to the given OPL2 channel.
///
/// Produces a moderate envelope (fast attack, no decay, full sustain) suitable
/// for verifying note-on/note-off behaviour without real GENMIDI data.
fn apply_default_instrument(opl: &mut OplChip, ch: u8) {
    let mod_reg = opl2_mod_reg(ch);
    let car_reg = opl2_car_reg(ch);

    // Modulator operator
    opl.write(0x20 + mod_reg, 0x01); // MULT=1, no tremolo/vibrato/sustain/KSR
    opl.write(0x40 + mod_reg, 0x10); // TL=16 (moderate attenuation)
    opl.write(0x60 + mod_reg, 0xF0); // AR=15, DR=0
    opl.write(0x80 + mod_reg, 0x00); // SL=0, RR=0

    // Carrier operator
    opl.write(0x20 + car_reg, 0x01); // MULT=1
    opl.write(0x40 + car_reg, 0x00); // TL=0 (full output volume)
    opl.write(0x60 + car_reg, 0xF0); // AR=15, DR=0
    opl.write(0x80 + car_reg, 0x00); // SL=0, RR=0

    // Channel: FM synthesis mode, 1 feedback bit
    opl.write(0xC0 + ch, 0x01);
}

// ---------------------------------------------------------------------------
// MidiPlayer
// ---------------------------------------------------------------------------

/// Drives an [`OplChip`] from a stream of [`MusEvent`] values.
///
/// Channel allocation uses round-robin assignment of the 9 OPL2 hardware
/// channels.  MUS channel 15 (percussion) is treated as a no-op.
pub struct MidiPlayer {
    /// The OPL2 chip register state.
    pub opl: OplChip,
    /// Map from MUS channel index → allocated OPL2 channel (0–8).
    /// `0xFF` means the MUS channel has not yet been assigned.
    channel_map: [u8; 16],
    /// Next OPL2 channel to hand out (wraps modulo 9).
    next_opl_ch: u8,
    /// Playback rate in ticks per second (MUS default: 140 Hz).
    pub ticks_per_sec: u32,
}

impl Default for MidiPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl MidiPlayer {
    /// Create a new, silent `MidiPlayer`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            opl: OplChip::new(),
            channel_map: [0xFF; 16],
            next_opl_ch: 0,
            ticks_per_sec: 140,
        }
    }

    /// Process a single [`MusEvent`], writing the appropriate OPL2 registers.
    pub fn process_event(&mut self, event: &MusEvent) {
        match event {
            MusEvent::PlayNote { channel, note, volume } => {
                // MUS channel 15 is percussion — no melodic OPL channel.
                if *channel == 15 {
                    return;
                }
                let opl_ch = self.alloc_channel(*channel);
                apply_default_instrument(&mut self.opl, opl_ch);

                let vol = volume.unwrap_or(127);
                let (block, fnum) = note_to_block_fnum(*note);

                // Frequency low byte
                self.opl.write(0xA0 + opl_ch, (fnum & 0xFF) as u8);

                // Frequency high bits, block, and key-on (bit 5)
                let b0_val = ((block & 0x7) << 2) | (((fnum >> 8) as u8) & 0x3) | 0x20;
                self.opl.write(0xB0 + opl_ch, b0_val);

                // Carrier total level: 63 = silence, 0 = loudest.
                // Map vol (0–127) so that vol=127 → TL=0, vol=0 → TL=63.
                let tl = 63u8.saturating_sub(vol / 2);
                let car_reg = opl2_car_reg(opl_ch);
                self.opl.write(0x40 + car_reg, tl.min(63));
            }

            MusEvent::ReleaseNote { channel, note: _ } => {
                if *channel == 15 {
                    return;
                }
                let idx = *channel as usize;
                // Only act if this MUS channel has been allocated.
                if idx < 16 && self.channel_map[idx] != 0xFF {
                    let opl_ch = self.channel_map[idx];
                    // Clear key-on (bit 5) without disturbing freq/block bits.
                    let prev = self.opl.read(0xB0 + opl_ch);
                    self.opl.write(0xB0 + opl_ch, prev & !0x20);
                }
            }

            MusEvent::Controller { channel, controller, value } => {
                // MUS controller 3 = volume
                if *controller == 3 {
                    let idx = *channel as usize;
                    if idx < 16 && self.channel_map[idx] != 0xFF {
                        let opl_ch = self.channel_map[idx];
                        let tl = 63u8.saturating_sub(*value / 2);
                        let car_reg = opl2_car_reg(opl_ch);
                        self.opl.write(0x40 + car_reg, tl.min(63));
                    }
                }
            }

            // No OPL2 action required for these events.
            MusEvent::ScoreEnd
            | MusEvent::MeasureEnd
            | MusEvent::PitchWheel { .. }
            | MusEvent::SystemEvent { .. } => {}
        }
    }

    /// Play every event in a [`MusScore`] in order (timing is ignored).
    ///
    /// Returns the total number of events processed.
    pub fn play_score(&mut self, score: &MusScore) -> usize {
        for (_, event) in &score.events {
            self.process_event(event);
        }
        score.events.len()
    }

    /// Allocate or look up the OPL2 channel for the given MUS channel.
    ///
    /// Uses round-robin assignment across the 9 OPL2 hardware channels.
    fn alloc_channel(&mut self, mus_ch: u8) -> u8 {
        let idx = mus_ch as usize;
        if self.channel_map[idx] != 0xFF {
            return self.channel_map[idx];
        }
        let opl_ch = self.next_opl_ch % 9;
        self.next_opl_ch = (self.next_opl_ch + 1) % 9;
        self.channel_map[idx] = opl_ch;
        opl_ch
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mus::{MusEvent, MusHeader, MusScore};

    /// Build a minimal [`MusScore`] from a list of events (all at delta=0).
    fn make_score(events: Vec<MusEvent>) -> MusScore {
        MusScore {
            header: MusHeader {
                score_length: 0,
                score_start: 0,
                primary_channels: 1,
                secondary_channels: 0,
                instrument_count: 0,
            },
            instruments: Vec::new(),
            events: events.into_iter().map(|e| (0u32, e)).collect(),
        }
    }

    #[test]
    fn midi_new_player_all_channels_silent() {
        let player = MidiPlayer::new();
        for ch in 0..9 {
            assert!(
                !player.opl.channel(ch).key_on,
                "OPL channel {ch} should be silent on new MidiPlayer"
            );
        }
    }

    #[test]
    fn midi_play_note_sets_key_on() {
        let mut player = MidiPlayer::new();
        player.process_event(&MusEvent::PlayNote {
            channel: 0,
            note: 60,
            volume: Some(100),
        });
        // MUS channel 0 is allocated to OPL channel 0 by round-robin.
        assert!(
            player.opl.channel(0).key_on,
            "OPL channel 0 should have key_on after PlayNote"
        );
    }

    #[test]
    fn midi_release_note_clears_key_on() {
        let mut player = MidiPlayer::new();
        // Play note first so channel 0 → OPL channel 0 and key_on=true.
        player.process_event(&MusEvent::PlayNote {
            channel: 0,
            note: 60,
            volume: Some(100),
        });
        assert!(player.opl.channel(0).key_on, "key_on should be set after play");

        // Release the same note.
        player.process_event(&MusEvent::ReleaseNote { channel: 0, note: 60 });
        assert!(
            !player.opl.channel(0).key_on,
            "key_on should be cleared after ReleaseNote"
        );
    }

    #[test]
    fn midi_channel_15_is_skipped() {
        let mut player = MidiPlayer::new();
        player.process_event(&MusEvent::PlayNote {
            channel: 15,
            note: 60,
            volume: Some(127),
        });
        // No OPL channel should have key_on set.
        for ch in 0..9 {
            assert!(
                !player.opl.channel(ch).key_on,
                "OPL channel {ch} must not be keyed on for percussion channel 15"
            );
        }
    }

    #[test]
    fn midi_play_score_returns_event_count() {
        let mut player = MidiPlayer::new();
        let score = make_score(vec![
            MusEvent::PlayNote { channel: 0, note: 60, volume: Some(100) },
            MusEvent::ReleaseNote { channel: 0, note: 60 },
            MusEvent::ScoreEnd,
        ]);
        let count = player.play_score(&score);
        assert_eq!(count, 3, "play_score must return the number of events in the score");
    }
}
