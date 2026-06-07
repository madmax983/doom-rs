//! OPL2 FM register model and synthesis.
//!
//! Models the Yamaha OPL2 (YM3812) chip's register state, decoded per-channel
//! parameters, and FM audio synthesis.

// ---------------------------------------------------------------------------
// OPL2 operator slot mapping: register-offset → (channel index, operator index)
// Operator 0 = modulator, operator 1 = carrier (Doom convention)
// ---------------------------------------------------------------------------
// Valid offsets 0..=17 only.
const SLOT_MAP: [(usize, usize); 18] = [
    (0, 0),
    (1, 0),
    (2, 0),
    (0, 1),
    (1, 1),
    (2, 1),
    (3, 0),
    (4, 0),
    (5, 0),
    (3, 1),
    (4, 1),
    (5, 1),
    (6, 0),
    (7, 0),
    (8, 0),
    (6, 1),
    (7, 1),
    (8, 1),
];

/// OPL2 frequency multiplier table (index = MULT field 0–15).
const MULT: [f32; 16] = [
    0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 10.0, 12.0, 12.0, 15.0, 15.0,
];

// ---------------------------------------------------------------------------
// Synthesis state types (private)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum EnvPhase {
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Debug, Clone, Copy)]
struct OperatorState {
    phase_acc: u32,
    env_level: f32,
    env_phase: EnvPhase,
}

impl Default for OperatorState {
    fn default() -> Self {
        Self {
            phase_acc: 0,
            env_level: 0.0,
            env_phase: EnvPhase::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ChannelState {
    ops: [OperatorState; 2],
    was_keyon: bool,
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// State of one OPL2 operator (either modulator or carrier).
///
/// ## Examples
///
/// ```
/// use doom_audio::opl::OplOperator;
///
/// let mut op = OplOperator::default();
/// op.attack_rate = 15;
/// op.decay_rate = 0;
/// op.sustain_level = 0;
/// op.release_rate = 15;
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct OplOperator {
    /// Attack rate (bits 7-4 of 0x60+offset register).
    pub attack_rate: u8,
    /// Decay rate (bits 3-0 of 0x60+offset register).
    pub decay_rate: u8,
    /// Sustain level (bits 7-4 of 0x80+offset register).
    pub sustain_level: u8,
    /// Release rate (bits 3-0 of 0x80+offset register).
    pub release_rate: u8,
    /// Total level / attenuation (bits 5-0 of 0x40+offset register).
    pub total_level: u8,
    /// Frequency multiplier (bits 3-0 of 0x20+offset register).
    pub mult: u8,
    /// Waveform select (bits 2-0 of 0xE0+offset register).
    pub waveform: u8,
}

/// State of one of the 9 OPL2 voice channels.
///
/// ## Examples
///
/// ```
/// use doom_audio::opl::{OplChannel, OplOperator};
///
/// let mut ch = OplChannel::default();
/// ch.freq_low = 0x42;
/// ch.freq_high = 0x01;
/// ch.key_on = true;
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct OplChannel {
    /// Low 8 bits of the F-number (register 0xA0+ch).
    pub freq_low: u8,
    /// High 2 bits of the F-number + block (bits 2-1-0 of register 0xB0+ch).
    pub freq_high: u8,
    /// Key-on bit (bit 5 of register 0xB0+ch).
    pub key_on: bool,
    /// `[modulator, carrier]` operator states.
    pub operators: [OplOperator; 2],
}

/// Complete OPL2 chip register state with FM synthesis.
///
/// Call [`OplChip::write`] to update registers and have the decoded channel
/// state kept in sync automatically. Call [`OplChip::synthesize`] to generate
/// audio samples.
///
/// ## Examples
///
/// ```
/// use doom_audio::opl::OplChip;
///
/// let mut chip = OplChip::new();
/// // Turn on channel 0 with a basic beep
/// chip.write(0xA0, 172);
/// chip.write(0xB0, (4 << 2) | 0x20); // block 4, key on
///
/// let mut buf = [0.0f32; 128];
/// chip.synthesize(&mut buf, 44100);
/// ```
pub struct OplChip {
    /// Raw shadow of every register byte (256 entries).
    regs: [u8; 256],
    /// Decoded channel state derived from `regs`.
    channels: [OplChannel; 9],
    /// Per-channel synthesis state (phase accumulators + ADSR envelopes).
    synth: [ChannelState; 9],
}

impl Default for OplChip {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Synthesis helpers
// ---------------------------------------------------------------------------

/// Generate one sample from a waveform given a 32-bit phase accumulator.
///
/// The top 16 bits of `phase` give the position within the 0–65535 cycle.
fn opl_waveform(waveform: u8, phase: u32) -> f32 {
    let t = (phase >> 16) as u16;
    let angle = t as f32 / 65536.0 * std::f32::consts::TAU;
    match waveform {
        0 => angle.sin(),
        1 => {
            if t < 32768 {
                angle.sin()
            } else {
                0.0
            }
        }
        2 => angle.sin().abs(),
        3 => {
            if t < 16384 || (32768..49152).contains(&t) {
                angle.sin().abs()
            } else {
                0.0
            }
        }
        _ => angle.sin(),
    }
}

impl OplChip {
    /// Create a new, silent OPL2 chip (all registers zeroed).
    pub fn new() -> Self {
        Self {
            regs: [0u8; 256],
            channels: [OplChannel::default(); 9],
            synth: [ChannelState::default(); 9],
        }
    }

    /// Write `val` to OPL2 register `reg`, keeping decoded channel state in sync.
    pub fn write(&mut self, reg: u8, val: u8) {
        self.regs[reg as usize] = val;

        match reg {
            // ---------------------------------------------------------------
            // 0xA0–0xA8: freq_low (F-number low byte) for channels 0–8
            // ---------------------------------------------------------------
            0xA0..=0xA8 => {
                let ch = (reg - 0xA0) as usize;
                self.channels[ch].freq_low = val;
            }

            // ---------------------------------------------------------------
            // 0xB0–0xB8: freq_high + key_on for channels 0–8
            //   bit 5      = key_on
            //   bits 4-2   = block (octave)
            //   bits 1-0   = top 2 bits of F-number
            // ---------------------------------------------------------------
            0xB0..=0xB8 => {
                let ch = (reg - 0xB0) as usize;
                self.channels[ch].freq_high = val & 0x1F;
                self.channels[ch].key_on = (val & 0x20) != 0;
            }

            // ---------------------------------------------------------------
            // 0x20–0x35: mult (frequency multiplier), bits 3-0
            // ---------------------------------------------------------------
            0x20..=0x35 => {
                let offset = (reg - 0x20) as usize;
                if let Some(&(ch, op)) = SLOT_MAP.get(offset) {
                    self.channels[ch].operators[op].mult = val & 0x0F;
                }
            }

            // ---------------------------------------------------------------
            // 0x40–0x55: total_level, bits 5-0
            // ---------------------------------------------------------------
            0x40..=0x55 => {
                let offset = (reg - 0x40) as usize;
                if let Some(&(ch, op)) = SLOT_MAP.get(offset) {
                    self.channels[ch].operators[op].total_level = val & 0x3F;
                }
            }

            // ---------------------------------------------------------------
            // 0x60–0x75: attack_rate (bits 7-4) + decay_rate (bits 3-0)
            // ---------------------------------------------------------------
            0x60..=0x75 => {
                let offset = (reg - 0x60) as usize;
                if let Some(&(ch, op)) = SLOT_MAP.get(offset) {
                    self.channels[ch].operators[op].attack_rate = val >> 4;
                    self.channels[ch].operators[op].decay_rate = val & 0x0F;
                }
            }

            // ---------------------------------------------------------------
            // 0x80–0x95: sustain_level (bits 7-4) + release_rate (bits 3-0)
            // ---------------------------------------------------------------
            0x80..=0x95 => {
                let offset = (reg - 0x80) as usize;
                if let Some(&(ch, op)) = SLOT_MAP.get(offset) {
                    self.channels[ch].operators[op].sustain_level = val >> 4;
                    self.channels[ch].operators[op].release_rate = val & 0x0F;
                }
            }

            // ---------------------------------------------------------------
            // 0xE0–0xF5: waveform select, bits 2-0
            // ---------------------------------------------------------------
            0xE0..=0xF5 => {
                let offset = (reg - 0xE0) as usize;
                if let Some(&(ch, op)) = SLOT_MAP.get(offset) {
                    self.channels[ch].operators[op].waveform = val & 0x07;
                }
            }

            // Everything else: raw register is already stored above; nothing
            // additional to decode (global chip settings, rhythm mode, etc.).
            _ => {}
        }
    }

    /// Read the raw register byte at `reg` (shadow copy, not real hardware).
    #[must_use]
    pub fn read(&self, reg: u8) -> u8 {
        self.regs[reg as usize]
    }

    /// Return a reference to the decoded state of channel `ch` (0–8).
    ///
    /// # Panics
    /// Panics if `ch >= 9`.
    #[must_use]
    pub fn channel(&self, ch: usize) -> &OplChannel {
        &self.channels[ch]
    }

    /// Generate `buf.len()` mono f32 samples at `sample_rate` Hz.
    ///
    /// Advances ADSR envelopes and phase accumulators for all 9 channels,
    /// then mixes their outputs into `buf`.  Output range: approximately [-1.0, 1.0].
    pub fn synthesize(&mut self, buf: &mut [f32], sample_rate: u32) {
        let sr = sample_rate as f32;

        for s in buf.iter_mut() {
            let mut mix = 0.0f32;

            for ch_idx in 0..9usize {
                let ch = &self.channels[ch_idx];
                let st = &mut self.synth[ch_idx];

                // -- Key-on edge detection ------------------------------------
                let key_on = ch.key_on;
                if key_on && !st.was_keyon {
                    // Rising edge: start attack on both operators.
                    for op in &mut st.ops {
                        op.env_phase = EnvPhase::Attack;
                        op.env_level = 0.0;
                        op.phase_acc = 0;
                    }
                } else if !key_on && st.was_keyon {
                    // Falling edge: start release on both operators.
                    for op in &mut st.ops {
                        if op.env_phase != EnvPhase::Off {
                            op.env_phase = EnvPhase::Release;
                        }
                    }
                }
                st.was_keyon = key_on;

                // -- Compute per-channel frequency ----------------------------
                let fnum = ((ch.freq_high as u16 & 0x03) << 8) | ch.freq_low as u16;
                let block = (ch.freq_high >> 2) & 0x07;
                // freq_hz = fnum * 49716 / 2^(20-block)
                let shift = 20u32.saturating_sub(block as u32);
                let freq_hz = fnum as f32 * 49716.0 / (1u32 << shift.min(30)) as f32;

                // -- Process each operator ------------------------------------
                let mut op_outputs = [0.0f32; 2];
                for (op_idx, op_output) in op_outputs.iter_mut().enumerate() {
                    let op_reg = &ch.operators[op_idx];
                    let op_st = &mut st.ops[op_idx];

                    if op_st.env_phase == EnvPhase::Off {
                        continue;
                    }

                    // ADSR envelope (linear approximation).
                    let sustain_target = 1.0 - op_reg.sustain_level as f32 / 15.0;
                    match op_st.env_phase {
                        EnvPhase::Attack => {
                            let ar = op_reg.attack_rate as f32;
                            op_st.env_level += ar * 4.0 / sr;
                            if op_st.env_level >= 1.0 {
                                op_st.env_level = 1.0;
                                op_st.env_phase = EnvPhase::Decay;
                            }
                        }
                        EnvPhase::Decay => {
                            let dr = op_reg.decay_rate as f32;
                            op_st.env_level -= dr * 2.0 / sr;
                            if op_st.env_level <= sustain_target {
                                op_st.env_level = sustain_target;
                                op_st.env_phase = EnvPhase::Sustain;
                            }
                        }
                        EnvPhase::Sustain => {
                            op_st.env_level = sustain_target;
                        }
                        EnvPhase::Release => {
                            let rr = op_reg.release_rate as f32;
                            op_st.env_level -= rr * 2.0 / sr;
                            if op_st.env_level <= 0.0 {
                                op_st.env_level = 0.0;
                                op_st.env_phase = EnvPhase::Off;
                                continue;
                            }
                        }
                        EnvPhase::Off => continue,
                    }

                    // Phase increment.
                    let mult = MULT[op_reg.mult as usize & 0x0F];
                    let op_freq = freq_hz * mult;
                    let phase_inc = (op_freq / sr * 4_294_967_296.0) as u32;
                    op_st.phase_acc = op_st.phase_acc.wrapping_add(phase_inc);

                    *op_output = opl_waveform(op_reg.waveform, op_st.phase_acc) * op_st.env_level;
                }

                // -- 2-operator FM: modulator → carrier phase modulation ------
                let mod_depth = (63i32 - ch.operators[0].total_level as i32).max(0) as f32 / 63.0;
                let fm_offset = (op_outputs[0] * mod_depth * 65536.0 * 4.0) as i32 as u32;
                let car_phase = self.synth[ch_idx].ops[1].phase_acc.wrapping_add(fm_offset);
                let car_waveform = ch.operators[1].waveform;
                let car_env = self.synth[ch_idx].ops[1].env_level;
                let car_vol = (63i32 - ch.operators[1].total_level as i32).max(0) as f32 / 63.0;
                let channel_out = if self.synth[ch_idx].ops[1].env_phase != EnvPhase::Off {
                    opl_waveform(car_waveform, car_phase) * car_env * car_vol
                } else {
                    0.0
                };

                mix += channel_out;
            }

            // Clamp the mixed output — no per-channel division, matching real
            // OPL2 DAC saturation behaviour.  Dividing by 9 makes music 9×
            // too quiet and inaudible beneath SFX.
            *s = mix.clamp(-1.0, 1.0);
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
    fn opl_new_is_silent() {
        let chip = OplChip::new();
        for ch in 0..9 {
            assert!(
                !chip.channel(ch).key_on,
                "channel {ch} should be silent on init"
            );
        }
    }

    #[test]
    fn opl_write_freq_low() {
        let mut chip = OplChip::new();
        chip.write(0xA0, 0x42);
        assert_eq!(chip.read(0xA0), 0x42);
        assert_eq!(chip.channel(0).freq_low, 0x42);
    }

    #[test]
    fn opl_write_key_on() {
        let mut chip = OplChip::new();
        chip.write(0xB0, 0x20);
        assert!(chip.channel(0).key_on);
    }

    #[test]
    fn opl_write_key_off() {
        let mut chip = OplChip::new();
        chip.write(0xB0, 0x20);
        assert!(chip.channel(0).key_on);
        chip.write(0xB0, 0x00);
        assert!(!chip.channel(0).key_on);
    }

    #[test]
    fn opl_write_total_level() {
        let mut chip = OplChip::new();
        chip.write(0x40, 0x3F);
        assert_eq!(chip.channel(0).operators[0].total_level, 0x3F);
    }

    #[test]
    fn opl_silent_chip_produces_zeros() {
        let mut chip = OplChip::new();
        let mut buf = vec![1.0f32; 64];
        chip.synthesize(&mut buf, 44_100);
        assert!(
            buf.iter().all(|&s| s == 0.0),
            "silent chip must output all zeros"
        );
    }

    #[test]
    fn opl_keyon_produces_nonzero_output() {
        let mut chip = OplChip::new();
        // Fast attack, no decay, full volume carrier.
        chip.write(0x60, 0xF0); // mod AR=15, DR=0
        chip.write(0x63, 0xF0); // car AR=15, DR=0
        chip.write(0x40, 0x00); // mod TL=0
        chip.write(0x43, 0x00); // car TL=0
        // F-number for ~440 Hz at block 4: fnum≈172
        chip.write(0xA0, 172);
        chip.write(0xB0, (4 << 2) | 0x20); // block=4, key_on=1
        let mut buf = vec![0.0f32; 100];
        chip.synthesize(&mut buf, 44_100);
        assert!(
            buf.iter().any(|&s| s != 0.0),
            "keyed-on channel must produce non-zero output"
        );
    }

    #[test]
    fn opl_keyoff_decays_to_zero() {
        let mut chip = OplChip::new();
        // Fast attack + fast release.
        chip.write(0x60, 0xF0); // AR=15, DR=0
        chip.write(0x63, 0xF0);
        chip.write(0x80, 0x0F); // SL=0, RR=15
        chip.write(0x83, 0x0F);
        chip.write(0x40, 0x00);
        chip.write(0x43, 0x00);
        chip.write(0xA0, 172);
        chip.write(0xB0, (4 << 2) | 0x20);
        // Sustain for 100 samples.
        let mut buf = vec![0.0f32; 100];
        chip.synthesize(&mut buf, 44_100);
        // Key off.
        chip.write(0xB0, 4 << 2); // key_on=0
        // Release for 1 second — with RR=15 and linear model, decays fast.
        let mut tail = vec![0.0f32; 44_100];
        chip.synthesize(&mut tail, 44_100);
        let last: &[f32] = &tail[tail.len() - 100..];
        for &s in last {
            assert!(
                s.abs() < 0.001,
                "output must decay to near-zero after key-off, got {s}"
            );
        }
    }

    #[test]
    fn opl_phase_advances() {
        let mut chip = OplChip::new();
        chip.write(0x60, 0xF0);
        chip.write(0x63, 0xF0);
        chip.write(0x40, 0x00);
        chip.write(0x43, 0x00);
        chip.write(0xA0, 172);
        chip.write(0xB0, (4 << 2) | 0x20);
        let mut buf = vec![0.0f32; 256];
        chip.synthesize(&mut buf, 44_100);
        let first = buf[0];
        assert!(
            buf.iter().any(|&s| s != first),
            "phase must advance — not all samples identical"
        );
    }

    #[test]
    fn opl_nine_channels_mix() {
        let mut chip = OplChip::new();
        // Key on all 9 channels with distinct F-numbers.
        for ch in 0..9u8 {
            chip.write(0x60 + ch, 0xF0);
            chip.write(0x63 + ch, 0xF0);
            chip.write(0x40 + ch, 0x00);
            chip.write(0x43 + ch, 0x00);
            chip.write(0xA0 + ch, 100 + ch * 10);
            chip.write(0xB0 + ch, (4 << 2) | 0x20);
        }
        let mut buf = vec![0.0f32; 64];
        chip.synthesize(&mut buf, 44_100);
        assert!(
            buf.iter().any(|&s| s != 0.0),
            "nine keyed-on channels must produce non-zero output"
        );
    }
}
