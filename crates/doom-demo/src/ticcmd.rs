//! Demo-format tic command: 4 bytes per player per tic.
//!
//! The LMP format stores only `forward_move` (i8), `side_move` (i8), and
//! `angle_turn` (i16 little-endian) per player per tic. The full game
//! `TicCmd` has additional `buttons` and `chatchar` fields that are NOT
//! part of the LMP wire format.

use doom_types::TicCmd;

/// Size of a single demo tic command in the LMP wire format.
pub const DEMO_TIC_SIZE: usize = 4;

/// 4-byte per-player per-tic entry in the LMP stream.
///
/// Unlike the full [`TicCmd`], the LMP format only stores movement and
/// angle — no buttons or chat character.
///
/// ```text
/// byte 0:   forward_move (i8)
/// byte 1:   side_move (i8)
/// byte 2-3: angle_turn (i16, little-endian)
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DemoTicCmd {
    /// Forward/backward movement (-128..127, positive = forward).
    pub forward_move: i8,
    /// Lateral strafe (-128..127, positive = right).
    pub side_move: i8,
    /// Angle delta in 16-bit BAM units.
    pub angle_turn: i16,
}

impl DemoTicCmd {
    /// Serialize this command to its 4-byte wire representation.
    #[must_use]
    pub const fn to_bytes(&self) -> [u8; DEMO_TIC_SIZE] {
        let angle_bytes = self.angle_turn.to_le_bytes();
        [
            self.forward_move as u8,
            self.side_move as u8,
            angle_bytes[0],
            angle_bytes[1],
        ]
    }

    /// Parse a 4-byte slice into a [`DemoTicCmd`].
    ///
    /// Returns `None` if `data.len() < 4`.
    #[must_use]
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < DEMO_TIC_SIZE {
            return None;
        }
        Some(Self {
            forward_move: data[0].cast_signed(),
            side_move: data[1].cast_signed(),
            angle_turn: i16::from_le_bytes([data[2], data[3]]),
        })
    }

    /// Convert a full [`TicCmd`] to a [`DemoTicCmd`], discarding buttons and
    /// chatchar (which are not stored in the LMP format).
    #[must_use]
    pub const fn from_ticcmd(cmd: &TicCmd) -> Self {
        Self {
            forward_move: cmd.forward_move,
            side_move: cmd.side_move,
            angle_turn: cmd.angle_turn,
        }
    }

    /// Convert this [`DemoTicCmd`] to a full [`TicCmd`].
    ///
    /// The `buttons`, `chatchar`, and padding fields default to zero.
    #[must_use]
    pub fn to_ticcmd(&self) -> TicCmd {
        TicCmd {
            forward_move: self.forward_move,
            side_move: self.side_move,
            angle_turn: self.angle_turn,
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use doom_types::TicCmd;

    // Test 7: to_bytes produces 4 bytes
    #[test]
    fn to_bytes_produces_4_bytes() {
        let cmd = DemoTicCmd {
            forward_move: 50,
            side_move: -10,
            angle_turn: 300,
        };
        let bytes = cmd.to_bytes();
        assert_eq!(bytes.len(), DEMO_TIC_SIZE);
    }

    // Test 8: from_bytes roundtrip
    #[test]
    fn from_bytes_roundtrip() {
        let original = DemoTicCmd {
            forward_move: 50,
            side_move: -10,
            angle_turn: 300,
        };
        let bytes = original.to_bytes();
        let parsed = DemoTicCmd::from_bytes(&bytes).expect("parse should succeed");
        assert_eq!(original, parsed);
    }

    // Test 9: to TicCmd conversion (buttons/chatchar zero)
    #[test]
    fn to_ticcmd_zeros_buttons_and_chatchar() {
        let demo_cmd = DemoTicCmd {
            forward_move: 25,
            side_move: -5,
            angle_turn: 100,
        };
        let tic = demo_cmd.to_ticcmd();
        assert_eq!(tic.forward_move, 25);
        assert_eq!(tic.side_move, -5);
        assert_eq!(tic.angle_turn, 100);
        assert_eq!(tic.buttons, 0);
        assert_eq!(tic.chatchar, 0);
    }

    // Test 10: TicCmd to DemoTicCmd conversion (strips buttons/chatchar)
    #[test]
    fn from_ticcmd_strips_buttons_and_chatchar() {
        let tic = TicCmd {
            forward_move: 30,
            side_move: -20,
            angle_turn: 500,
            buttons: 0xFF,
            chatchar: b'A',
            ..Default::default()
        };

        let demo_cmd = DemoTicCmd::from_ticcmd(&tic);
        assert_eq!(demo_cmd.forward_move, 30);
        assert_eq!(demo_cmd.side_move, -20);
        assert_eq!(demo_cmd.angle_turn, 500);
    }

    // Test 11: Negative angle_turn survives roundtrip
    #[test]
    fn negative_angle_turn_survives_roundtrip() {
        let original = DemoTicCmd {
            forward_move: 0,
            side_move: 0,
            angle_turn: -1234,
        };
        let bytes = original.to_bytes();
        let parsed = DemoTicCmd::from_bytes(&bytes).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn from_bytes_too_short_returns_none() {
        assert!(DemoTicCmd::from_bytes(&[]).is_none());
        assert!(DemoTicCmd::from_bytes(&[0, 1, 2]).is_none());
    }

    #[test]
    fn extreme_values_roundtrip() {
        let original = DemoTicCmd {
            forward_move: i8::MIN,
            side_move: i8::MAX,
            angle_turn: i16::MIN,
        };
        let bytes = original.to_bytes();
        let parsed = DemoTicCmd::from_bytes(&bytes).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn default_is_all_zeros() {
        let cmd = DemoTicCmd::default();
        assert_eq!(cmd.forward_move, 0);
        assert_eq!(cmd.side_move, 0);
        assert_eq!(cmd.angle_turn, 0);
    }

    #[test]
    fn ticcmd_roundtrip_preserves_movement_and_angle() {
        let tic = TicCmd {
            forward_move: -50,
            side_move: 40,
            angle_turn: -8000,
            buttons: 0x03,
            ..Default::default()
        };

        let demo = DemoTicCmd::from_ticcmd(&tic);
        let back = demo.to_ticcmd();

        assert_eq!(back.forward_move, -50);
        assert_eq!(back.side_move, 40);
        assert_eq!(back.angle_turn, -8000);
        // buttons are NOT preserved
        assert_eq!(back.buttons, 0);
    }
}
