//! OPL2 FM register model.
//!
//! Models the Yamaha OPL2 (YM3812) chip's register state and the decoded
//! per-channel, per-operator parameters that follow from it.  No audio
//! synthesis is performed here — this is a pure register-state tracker.

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

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// State of one OPL2 operator (either modulator or carrier).
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

/// Complete OPL2 chip register state.
///
/// Call [`OplChip::write`] to update registers and have the decoded channel
/// state kept in sync automatically.
pub struct OplChip {
    /// Raw shadow of every register byte (256 entries).
    regs: [u8; 256],
    /// Decoded channel state derived from `regs`.
    channels: [OplChannel; 9],
}

impl Default for OplChip {
    fn default() -> Self {
        Self::new()
    }
}

impl OplChip {
    /// Create a new, silent OPL2 chip (all registers zeroed).
    pub fn new() -> Self {
        Self {
            regs: [0u8; 256],
            channels: [OplChannel::default(); 9],
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
            //   bits 2-0   = top 2 bits of F-number + block
            // ---------------------------------------------------------------
            0xB0..=0xB8 => {
                let ch = (reg - 0xB0) as usize;
                self.channels[ch].freq_high = val & 0x1F; // keep bottom 5 bits
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
            assert!(!chip.channel(ch).key_on, "channel {ch} should be silent on init");
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
        // Register 0x40, offset 0 → SLOT_MAP[0] = (channel 0, operator 0)
        chip.write(0x40, 0x3F);
        assert_eq!(chip.channel(0).operators[0].total_level, 0x3F);
    }
}
