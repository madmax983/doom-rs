//! MUS→OPL2 sequencer.
//!
//! Drives an [`OplChip`] from a decoded [`MusScore`].  Maps MUS channels to
//! OPL2 hardware channels and translates [`MusEvent`] values into OPL2
//! register writes.
//!
//! # Timed playback
//!
//! Call [`MidiPlayer::load_score`] to queue a score for timed playback, then
//! call [`MidiPlayer::advance_samples`] from the audio callback on every
//! buffer fill.  The sequencer tracks accumulated sample position and fires
//! events when their absolute tick position becomes due.  The score loops
//! automatically when [`MusEvent::ScoreEnd`] is reached.

use crate::{
    AudioError,
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
// GENMIDI instrument bank
// ---------------------------------------------------------------------------

/// One OPL2 voice (operator pair) from a GENMIDI instrument record.
///
/// Each field corresponds directly to an OPL2 register write as described
/// in the GENMIDI lump specification.
#[derive(Debug, Clone, Copy, Default)]
pub struct GenmidiVoice {
    // Modulator operator
    /// Written to OPL2 register `0x20 + mod_slot`.
    pub mod_trem_vibrato: u8,
    /// Written to OPL2 register `0x60 + mod_slot`.
    pub mod_attack_decay: u8,
    /// Written to OPL2 register `0x80 + mod_slot`.
    pub mod_sustain_release: u8,
    /// Written to OPL2 register `0xE0 + mod_slot`.
    pub mod_wave_select: u8,
    /// Written to OPL2 register `0x40 + mod_slot`.
    pub mod_ksl_output: u8,
    /// Written to OPL2 register `0xC0 + ch` (feedback/connection byte).
    pub feedback: u8,
    // Carrier operator
    /// Written to OPL2 register `0x20 + car_slot`.
    pub car_trem_vibrato: u8,
    /// Written to OPL2 register `0x60 + car_slot`.
    pub car_attack_decay: u8,
    /// Written to OPL2 register `0x80 + car_slot`.
    pub car_sustain_release: u8,
    /// Written to OPL2 register `0xE0 + car_slot`.
    pub car_wave_select: u8,
    /// Written to OPL2 register `0x40 + car_slot`.
    pub car_ksl_output: u8,
    /// Padding byte — ignored.
    pub _unused: u8,
    /// Semitone offset added to the MIDI note before frequency lookup.
    pub base_note_offset: i16,
}

/// One instrument definition from the GENMIDI lump.
///
/// Each record is 32 bytes on disk: 4 bytes of header fields followed by
/// two 14-byte voice records.
#[derive(Debug, Clone)]
pub struct GenmidiInstrument {
    /// Instrument flags.
    ///
    /// - Bit 0: fixed pitch — use `fixed_note` instead of the MIDI note.
    /// - Bit 1: two-voice instrument — voice\[1\] is valid (usually ignored).
    pub flags: u16,
    /// Fine-tuning byte (rarely used; stored but not applied).
    pub fine_tuning: u8,
    /// Note to play when `flags & 1` is set (fixed-pitch instruments).
    pub fixed_note: u8,
    /// Primary (voice\[0\]) and secondary (voice\[1\]) operator pairs.
    pub voices: [GenmidiVoice; 2],
}

/// Parsed GENMIDI WAD lump — 175 GM instrument patches for OPL2.
///
/// Indices 0–127 are melodic GM patches; 128–174 are percussion.
///
/// # Example
///
/// ```rust,no_run
/// # use doom_audio::midi::GenmidiBank;
/// // `data` is the raw bytes of the GENMIDI lump from the WAD.
/// # let data = vec![0u8; 5608]; // placeholder
/// let bank = GenmidiBank::parse(&data).expect("valid GENMIDI lump");
/// let piano = bank.get(0); // Acoustic Grand Piano
/// let _ = piano.voices[0].feedback;
/// ```
pub struct GenmidiBank {
    /// 175 instrument definitions indexed by GM patch number.
    pub instruments: Vec<GenmidiInstrument>,
}

impl GenmidiBank {
    /// Parse raw GENMIDI lump bytes.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::InvalidMus`] (repurposed) if:
    /// - `data` is shorter than `8 + 175 * 32 = 5608` bytes, or
    /// - the 8-byte magic header is not `b"#OPL_II#"`.
    pub fn parse(data: &[u8]) -> Result<Self, AudioError> {
        const HEADER: &[u8; 8] = b"#OPL_II#";
        const INSTR_SIZE: usize = 32;
        const NUM_INSTRS: usize = 175;
        const MIN_LEN: usize = 8 + INSTR_SIZE * NUM_INSTRS; // = 5608

        if data.len() < MIN_LEN {
            return Err(AudioError::InvalidMus("GENMIDI lump too short"));
        }
        if &data[0..8] != HEADER {
            return Err(AudioError::InvalidMus("invalid GENMIDI header"));
        }

        let mut instruments = Vec::with_capacity(NUM_INSTRS);
        for i in 0..NUM_INSTRS {
            let base = 8 + i * INSTR_SIZE;
            let b = &data[base..base + INSTR_SIZE];
            let flags = u16::from_le_bytes([b[0], b[1]]);
            let fine_tuning = b[2];
            let fixed_note = b[3];
            let v0 = parse_genmidi_voice(&b[4..18]);
            let v1 = parse_genmidi_voice(&b[18..32]);
            instruments.push(GenmidiInstrument {
                flags,
                fine_tuning,
                fixed_note,
                voices: [v0, v1],
            });
        }
        Ok(Self { instruments })
    }

    /// Get the instrument for a GM program number (0–174).
    ///
    /// Out-of-range indices are clamped to the last valid index (174) —
    /// this method never panics.
    pub fn get(&self, program: usize) -> &GenmidiInstrument {
        let idx = program.min(self.instruments.len().saturating_sub(1));
        &self.instruments[idx]
    }
}

/// Parse a 14-byte GENMIDI voice record from `b`.
fn parse_genmidi_voice(b: &[u8]) -> GenmidiVoice {
    GenmidiVoice {
        mod_trem_vibrato: b[0],
        mod_attack_decay: b[1],
        mod_sustain_release: b[2],
        mod_wave_select: b[3],
        mod_ksl_output: b[4],
        feedback: b[5],
        car_trem_vibrato: b[6],
        car_attack_decay: b[7],
        car_sustain_release: b[8],
        car_wave_select: b[9],
        car_ksl_output: b[10],
        _unused: b[11],
        base_note_offset: i16::from_le_bytes([b[12], b[13]]),
    }
}

/// Write a GENMIDI voice's register values to the given OPL2 channel.
fn apply_genmidi_instrument(opl: &mut OplChip, opl_ch: u8, voice: &GenmidiVoice) {
    let mod_reg = opl2_mod_reg(opl_ch);
    let car_reg = opl2_car_reg(opl_ch);

    // Modulator operator registers
    opl.write(0x20 + mod_reg, voice.mod_trem_vibrato);
    opl.write(0x40 + mod_reg, voice.mod_ksl_output);
    opl.write(0x60 + mod_reg, voice.mod_attack_decay);
    opl.write(0x80 + mod_reg, voice.mod_sustain_release);
    opl.write(0xE0 + mod_reg, voice.mod_wave_select);

    // Carrier operator registers
    opl.write(0x20 + car_reg, voice.car_trem_vibrato);
    opl.write(0x40 + car_reg, voice.car_ksl_output);
    opl.write(0x60 + car_reg, voice.car_attack_decay);
    opl.write(0x80 + car_reg, voice.car_sustain_release);
    opl.write(0xE0 + car_reg, voice.car_wave_select);

    // Feedback / connection register (per-channel)
    opl.write(0xC0 + opl_ch, voice.feedback);
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Discriminant returned by the borrow-splitting logic inside
/// [`MidiPlayer::advance_samples`].
enum LoopAction {
    /// No event is due in this window — exit the loop.
    Break,
    /// The score has been exhausted; restart it.
    Restart {
        /// Absolute tick number of the first event after restart.
        restart_tick: u64,
        /// Delta of `events[0]` (added to `restart_tick` to get first event's due tick).
        first_delta: u64,
    },
    /// An event is due — process it.
    Process {
        event_clone: MusEvent,
        delta_next: Option<u64>,
    },
}

// ---------------------------------------------------------------------------
// MidiPlayer
// ---------------------------------------------------------------------------

/// Drives an [`OplChip`] from a stream of [`MusEvent`] values.
///
/// Channel allocation uses round-robin assignment of the 9 OPL2 hardware
/// channels.  MUS channel 15 (percussion) is treated as a no-op.
///
/// For timed playback, load a score with [`MidiPlayer::load_score`] and call
/// [`MidiPlayer::advance_samples`] from the audio callback on every buffer
/// fill.  The sequencer loops automatically when [`MusEvent::ScoreEnd`] is
/// reached.
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

    // -----------------------------------------------------------------------
    // Timed-playback state
    // -----------------------------------------------------------------------
    /// The currently loaded score, if any.
    pub current_score: Option<MusScore>,
    /// Index into `current_score.events` for the next unplayed event.
    pub event_cursor: usize,
    /// Absolute tick number at which `events[event_cursor]` becomes due.
    ///
    /// This is the *cumulative* sum of all delta values from event 0 up to
    /// (but not including) `event_cursor`.
    pub next_event_tick: u64,
    /// Accumulated sample count since the score was loaded.
    sample_count: u64,

    // -----------------------------------------------------------------------
    // GENMIDI state
    // -----------------------------------------------------------------------
    /// Loaded GENMIDI patch bank, if any.
    ///
    /// When `Some`, real FM instrument patches are applied on each `PlayNote`
    /// event instead of the built-in sine-wave default.
    pub genmidi: Option<GenmidiBank>,
    /// Current GM program (instrument patch) per MUS channel (0–15).
    ///
    /// Updated by `Controller { controller: 2, value }` events.
    channel_program: [u8; 16],
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
            current_score: None,
            event_cursor: 0,
            next_event_tick: 0,
            sample_count: 0,
            genmidi: None,
            channel_program: [0u8; 16],
        }
    }

    /// Load a GENMIDI patch bank for real FM instrument sounds.
    ///
    /// After calling this, `PlayNote` events will apply the appropriate GM
    /// patch from `bank` instead of the built-in sine-wave default.
    pub fn load_genmidi(&mut self, bank: GenmidiBank) {
        self.genmidi = Some(bank);
    }

    /// Load a new score for timed playback, resetting position to the start.
    ///
    /// Silences the OPL chip and resets all channel allocations before
    /// beginning playback of the new score.  Per-channel GM program numbers
    /// are also reset to 0 (Acoustic Grand Piano).
    pub fn load_score(&mut self, score: MusScore) {
        // Silence the chip and reset channel allocations.
        self.opl = OplChip::new();
        self.channel_map = [0xFF; 16];
        self.next_opl_ch = 0;
        self.channel_program = [0u8; 16];

        // The first event is due immediately (its delta is the delay *before*
        // it fires, so for event 0 the absolute tick = events[0].0).
        let first_tick = score.events.first().map_or(0, |e| u64::from(e.0));

        self.current_score = Some(score);
        self.event_cursor = 0;
        self.next_event_tick = first_tick;
        self.sample_count = 0;
    }

    /// Stop playback and silence all OPL channels.
    pub fn stop(&mut self) {
        self.current_score = None;
        self.opl = OplChip::new();
        self.channel_map = [0xFF; 16];
        self.next_opl_ch = 0;
        self.sample_count = 0;
        self.event_cursor = 0;
        self.next_event_tick = 0;
    }

    /// Advance playback by `n_samples` at `sample_rate` Hz, synthesizing OPL
    /// audio into `buf`.
    ///
    /// - Processes all MUS events whose absolute tick position falls within
    ///   the current window `[sample_count, sample_count + n_samples)`.
    /// - Calls `self.opl.synthesize(buf, sample_rate)` to fill `buf` with OPL
    ///   audio after event processing.
    /// - Loops the score automatically when [`MusEvent::ScoreEnd`] is reached.
    /// - If no score is loaded, `buf` is filled with zeros.
    ///
    /// `buf.len()` must equal `n_samples`.
    pub fn advance_samples(&mut self, n_samples: usize, sample_rate: u32, buf: &mut [f32]) {
        // Zero the output buffer first.
        for s in buf.iter_mut() {
            *s = 0.0;
        }

        let sample_count_end = self.sample_count + n_samples as u64;

        // Process all events that are due before sample_count_end.
        // `next_event_tick` is an ever-increasing absolute tick counter that
        // never wraps on loop — when the score restarts we add the score's
        // total tick duration to next_event_tick rather than resetting it.
        // This prevents the loop from firing the same event infinitely.
        loop {
            if self.current_score.is_none() {
                break;
            }

            // Gather what we need in a short immutable-borrow block.
            let action = {
                let score = self.current_score.as_ref().expect("checked above");

                if self.event_cursor >= score.events.len() {
                    // Past the end of the event list — ScoreEnd was the last
                    // event and we've already processed it.  Restart.
                    //
                    // `next_event_tick` currently points to the tick of the
                    // last processed event.  The first event of the new loop
                    // is due at `next_event_tick + events[0].delta`.  We keep
                    // `next_event_tick` monotonically increasing so no event
                    // fires more than once per loop iteration.
                    let first_delta = score.events.first().map_or(0, |e| u64::from(e.0));
                    LoopAction::Restart {
                        restart_tick: self.next_event_tick,
                        first_delta,
                    }
                } else {
                    // Convert this event's absolute tick to a sample position.
                    let event_sample = (self.next_event_tick as u128 * sample_rate as u128
                        / self.ticks_per_sec as u128) as u64;

                    if event_sample >= sample_count_end {
                        LoopAction::Break
                    } else {
                        let event_clone = score.events[self.event_cursor].1.clone();
                        let delta_next = score
                            .events
                            .get(self.event_cursor + 1)
                            .map(|e| u64::from(e.0));
                        LoopAction::Process {
                            event_clone,
                            delta_next,
                        }
                    }
                }
            };

            match action {
                LoopAction::Break => break,

                LoopAction::Restart {
                    restart_tick,
                    first_delta,
                } => {
                    self.event_cursor = 0;
                    // next_event_tick for the first event of the new loop.
                    // restart_tick is the current absolute tick (tick of the
                    // last processed event); first_delta is events[0].delta.
                    self.next_event_tick = restart_tick + first_delta;
                    // Always break after a restart.  Events from the new loop
                    // iteration will fire on the next advance_samples() call.
                    // This prevents infinite looping when all deltas are zero
                    // and keeps the timing error bounded to at most one buffer
                    // period (~1–5 ms at typical block sizes).
                    break;
                }

                LoopAction::Process {
                    event_clone,
                    delta_next,
                } => {
                    self.process_event(&event_clone);
                    self.event_cursor += 1;
                    if let Some(d) = delta_next {
                        self.next_event_tick += d;
                    }
                    // If there is no next event, next iteration will hit the
                    // Restart arm.
                }
            }
        }

        // Synthesize OPL audio for this window.
        // Agent 1 implements OplChip::synthesize; we call it here.
        self.opl.synthesize(buf, sample_rate);

        self.sample_count = sample_count_end;
    }

    /// Process a single [`MusEvent`], writing the appropriate OPL2 registers.
    pub fn process_event(&mut self, event: &MusEvent) {
        match event {
            MusEvent::PlayNote {
                channel,
                note,
                volume,
            } => {
                // MUS channel 15 is percussion — no melodic OPL channel.
                if *channel == 15 {
                    return;
                }
                let opl_ch = self.alloc_channel(*channel);

                // Apply instrument — GENMIDI if loaded, else default sine.
                // Compute the note offset from the GENMIDI bank at the same time.
                let genmidi_note_offset: i16 = if let Some(ref bank) = self.genmidi {
                    let prog = self.channel_program[*channel as usize & 0x0F] as usize;
                    let instr = bank.get(prog);
                    apply_genmidi_instrument(&mut self.opl, opl_ch, &instr.voices[0]);
                    instr.voices[0].base_note_offset
                } else {
                    apply_default_instrument(&mut self.opl, opl_ch);
                    0
                };

                let vol = volume.unwrap_or(127);

                // Apply base_note_offset from GENMIDI voice to the MIDI note.
                let actual_note = (*note as i16)
                    .saturating_add(genmidi_note_offset)
                    .clamp(0, 127) as u8;
                let (block, fnum) = note_to_block_fnum(actual_note);

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

            MusEvent::Controller {
                channel,
                controller,
                value,
            } => {
                let idx = *channel as usize & 0x0F;
                match *controller {
                    // MUS controller 2 = program/voice change.
                    2 => {
                        self.channel_program[idx] = *value;
                    }
                    // MUS controller 3 = volume.
                    3 => {
                        if self.channel_map[idx] != 0xFF {
                            let opl_ch = self.channel_map[idx];
                            let tl = 63u8.saturating_sub(*value / 2);
                            let car_reg = opl2_car_reg(opl_ch);
                            self.opl.write(0x40 + car_reg, tl.min(63));
                        }
                    }
                    _ => {}
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

    /// Build a [`MusScore`] from events with explicit per-event delta ticks.
    fn make_score_with_deltas(events: Vec<(u32, MusEvent)>) -> MusScore {
        MusScore {
            header: MusHeader {
                score_length: 0,
                score_start: 0,
                primary_channels: 1,
                secondary_channels: 0,
                instrument_count: 0,
            },
            instruments: Vec::new(),
            events,
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
        assert!(
            player.opl.channel(0).key_on,
            "key_on should be set after play"
        );

        // Release the same note.
        player.process_event(&MusEvent::ReleaseNote {
            channel: 0,
            note: 60,
        });
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
            MusEvent::PlayNote {
                channel: 0,
                note: 60,
                volume: Some(100),
            },
            MusEvent::ReleaseNote {
                channel: 0,
                note: 60,
            },
            MusEvent::ScoreEnd,
        ]);
        let count = player.play_score(&score);
        assert_eq!(
            count, 3,
            "play_score must return the number of events in the score"
        );
    }

    // -----------------------------------------------------------------------
    // Timed-playback tests
    // -----------------------------------------------------------------------

    #[test]
    fn midi_load_score_resets_state() {
        let mut player = MidiPlayer::new();
        // Fake prior state.
        player.event_cursor = 5;
        player.sample_count = 99_999;

        let score = make_score(vec![MusEvent::ScoreEnd]);
        player.load_score(score);

        assert_eq!(
            player.event_cursor, 0,
            "event_cursor must reset to 0 after load_score"
        );
        assert_eq!(
            player.sample_count, 0,
            "sample_count must reset to 0 after load_score"
        );
        assert!(
            player.current_score.is_some(),
            "current_score must be Some after load_score"
        );
    }

    #[test]
    fn midi_advance_samples_processes_events() {
        // Score: PlayNote at delta=0, ScoreEnd at delta=100.
        // With sample_rate=44100 and ticks_per_sec=140:
        //   PlayNote is due at tick 0 → sample 0.
        //   ScoreEnd is due at tick 100 → sample ~31500.
        // advance_samples(44100) covers the full second, so PlayNote fires.
        let mut player = MidiPlayer::new();
        let score = make_score_with_deltas(vec![
            (
                0,
                MusEvent::PlayNote {
                    channel: 0,
                    note: 60,
                    volume: Some(127),
                },
            ),
            (100, MusEvent::ScoreEnd),
        ]);
        player.load_score(score);

        let mut buf = vec![0.0f32; 44_100];
        player.advance_samples(44_100, 44_100, &mut buf);

        // OPL channel 0 should have key_on set after PlayNote was dispatched.
        assert!(
            player.opl.channel(0).key_on,
            "OPL channel 0 should have key_on after PlayNote event was processed"
        );
    }

    #[test]
    fn midi_advance_samples_future_event_not_fired() {
        // PlayNote is at tick 1000 (≈7143 samples at 44100/140).
        // advance_samples(64) should NOT fire it.
        let mut player = MidiPlayer::new();
        let score = make_score_with_deltas(vec![
            (
                1000,
                MusEvent::PlayNote {
                    channel: 0,
                    note: 60,
                    volume: Some(127),
                },
            ),
            (1, MusEvent::ScoreEnd),
        ]);
        player.load_score(score);

        let mut buf = vec![0.0f32; 64];
        player.advance_samples(64, 44_100, &mut buf);

        assert!(
            !player.opl.channel(0).key_on,
            "OPL channel 0 must NOT have key_on when event is far in the future"
        );
    }

    #[test]
    fn midi_stop_silences() {
        let mut player = MidiPlayer::new();
        let score = make_score(vec![
            MusEvent::PlayNote {
                channel: 0,
                note: 60,
                volume: Some(127),
            },
            MusEvent::ScoreEnd,
        ]);
        player.load_score(score);

        // Advance enough to fire all events.
        let mut buf = vec![0.0f32; 64];
        player.advance_samples(64, 44_100, &mut buf);

        player.stop();

        assert!(
            player.current_score.is_none(),
            "current_score must be None after stop()"
        );
        assert_eq!(
            player.event_cursor, 0,
            "event_cursor must be 0 after stop()"
        );
        assert_eq!(
            player.sample_count, 0,
            "sample_count must be 0 after stop()"
        );
        // OPL chip should be reset (all channels silent).
        for ch in 0..9 {
            assert!(
                !player.opl.channel(ch).key_on,
                "OPL channel {ch} must be silent after stop()"
            );
        }
    }

    #[test]
    fn midi_advance_samples_zeros_buf_when_no_score() {
        let mut player = MidiPlayer::new();
        let mut buf = vec![1.0f32; 64]; // pre-fill with non-zero
        player.advance_samples(64, 44_100, &mut buf);
        // The buffer should be all zeros (OPL silent, no score).
        assert!(
            buf.iter().all(|&s| s == 0.0),
            "advance_samples must zero the buffer when no score is loaded"
        );
    }

    // -----------------------------------------------------------------------
    // GENMIDI tests
    // -----------------------------------------------------------------------

    /// Build a valid 5608-byte GENMIDI buffer: magic header + 175 × 32 zero bytes.
    fn make_genmidi_buf() -> Vec<u8> {
        let mut buf = Vec::with_capacity(5608);
        buf.extend_from_slice(b"#OPL_II#");
        buf.extend(std::iter::repeat_n(0u8, 175 * 32));
        buf
    }

    #[test]
    fn genmidi_parse_valid_bank() {
        let buf = make_genmidi_buf();
        let bank = GenmidiBank::parse(&buf).expect("valid GENMIDI must parse");
        assert_eq!(
            bank.instruments.len(),
            175,
            "bank must contain exactly 175 instruments"
        );
    }

    #[test]
    fn genmidi_parse_bad_magic() {
        let mut buf = make_genmidi_buf();
        // Corrupt the first byte of the magic header.
        buf[0] = b'X';
        let result = GenmidiBank::parse(&buf);
        assert!(result.is_err(), "wrong magic header must produce an error");
    }

    #[test]
    fn genmidi_parse_too_short() {
        let buf = vec![0u8; 10];
        let result = GenmidiBank::parse(&buf);
        assert!(
            result.is_err(),
            "buffer shorter than 5608 bytes must produce an error"
        );
    }

    #[test]
    fn genmidi_get_clamps_out_of_range() {
        let buf = make_genmidi_buf();
        let bank = GenmidiBank::parse(&buf).expect("valid GENMIDI must parse");
        // get(999) must not panic and must return the last instrument (index 174).
        let instr = bank.get(999);
        // The returned reference must be the same as instruments[174].
        let last = bank.get(174);
        // Both should have the same flags (all zeros in our test buffer).
        assert_eq!(
            instr.flags, last.flags,
            "out-of-range get must clamp to last instrument"
        );
    }

    #[test]
    fn genmidi_apply_writes_registers() {
        // Build a bank where instrument 0 voice[0] has specific register values.
        let mut buf = make_genmidi_buf();
        // Instrument 0 starts at offset 8; voice[0] at offset 8+4 = 12.
        // Voice layout (14 bytes): [mod_tv, mod_ad, mod_sr, mod_ws, mod_ksl,
        //                           feedback, car_tv, car_ad, car_sr, car_ws,
        //                           car_ksl, _unused, base_lo, base_hi]
        let v_offset = 8 + 4; // = 12
        buf[v_offset] = 0x01; // mod_trem_vibrato
        buf[v_offset + 1] = 0xF0; // mod_attack_decay
        buf[v_offset + 2] = 0x0F; // mod_sustain_release
        buf[v_offset + 3] = 0x02; // mod_wave_select
        buf[v_offset + 4] = 0x10; // mod_ksl_output
        buf[v_offset + 5] = 0x0E; // feedback
        buf[v_offset + 6] = 0x03; // car_trem_vibrato
        buf[v_offset + 7] = 0xF1; // car_attack_decay
        buf[v_offset + 8] = 0x01; // car_sustain_release
        buf[v_offset + 9] = 0x01; // car_wave_select
        buf[v_offset + 10] = 0x00; // car_ksl_output
        // bytes 11–13 stay 0 (_unused and base_note_offset = 0)

        let bank = GenmidiBank::parse(&buf).expect("valid GENMIDI must parse");
        let mut opl = OplChip::new();
        apply_genmidi_instrument(&mut opl, 0, &bank.instruments[0].voices[0]);

        let mod_reg = opl2_mod_reg(0); // = 0
        let car_reg = opl2_car_reg(0); // = 3

        assert_eq!(
            opl.read(0x20 + mod_reg),
            0x01,
            "mod_trem_vibrato must be written to 0x20+mod"
        );
        assert_eq!(
            opl.read(0x40 + mod_reg),
            0x10,
            "mod_ksl_output must be written to 0x40+mod"
        );
        assert_eq!(
            opl.read(0x60 + mod_reg),
            0xF0,
            "mod_attack_decay must be written to 0x60+mod"
        );
        assert_eq!(
            opl.read(0x80 + mod_reg),
            0x0F,
            "mod_sustain_release must be written to 0x80+mod"
        );
        assert_eq!(
            opl.read(0xE0 + mod_reg),
            0x02,
            "mod_wave_select must be written to 0xE0+mod"
        );
        assert_eq!(
            opl.read(0x20 + car_reg),
            0x03,
            "car_trem_vibrato must be written to 0x20+car"
        );
        assert_eq!(
            opl.read(0x40 + car_reg),
            0x00,
            "car_ksl_output must be written to 0x40+car"
        );
        assert_eq!(
            opl.read(0x60 + car_reg),
            0xF1,
            "car_attack_decay must be written to 0x60+car"
        );
        assert_eq!(
            opl.read(0x80 + car_reg),
            0x01,
            "car_sustain_release must be written to 0x80+car"
        );
        assert_eq!(
            opl.read(0xE0 + car_reg),
            0x01,
            "car_wave_select must be written to 0xE0+car"
        );
        assert_eq!(opl.read(0xC0), 0x0E, "feedback must be written to 0xC0+ch");
    }

    #[test]
    fn genmidi_controller2_changes_program() {
        let mut player = MidiPlayer::new();
        // Send a Controller { controller: 2, value: 5 } on channel 0.
        player.process_event(&MusEvent::Controller {
            channel: 0,
            controller: 2,
            value: 5,
        });
        assert_eq!(
            player.channel_program[0], 5,
            "controller 2 must update channel_program"
        );
    }

    #[test]
    fn genmidi_playnote_uses_program() {
        // Build a bank where instrument 1 voice[0] has a distinctive feedback value.
        let mut buf = make_genmidi_buf();
        // Instrument 1, voice[0] starts at: 8 + 1*32 + 4 = 44
        let v_offset = 8 + 32 + 4;
        buf[v_offset + 5] = 0x3E; // feedback = 0x3E (distinctive)

        let bank = GenmidiBank::parse(&buf).expect("valid GENMIDI must parse");
        let mut player = MidiPlayer::new();
        player.load_genmidi(bank);

        // Switch channel 0 to program 1 via controller 2.
        player.process_event(&MusEvent::Controller {
            channel: 0,
            controller: 2,
            value: 1,
        });

        // Play a note — this should apply instrument 1's registers.
        player.process_event(&MusEvent::PlayNote {
            channel: 0,
            note: 60,
            volume: Some(127),
        });

        // OPL channel 0 is allocated for MUS channel 0.
        // Check that the feedback register (0xC0 + opl_ch = 0xC0) matches instrument 1.
        assert_eq!(
            player.opl.read(0xC0),
            0x3E,
            "PlayNote with GENMIDI must apply the instrument for the current program"
        );
    }
}
