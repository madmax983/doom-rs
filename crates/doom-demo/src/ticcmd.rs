//! Demo-format tic command: 4 bytes per player per tic.
//!
//! The LMP format stores `forward_move`, `side_move`, a one-byte turn value,
//! and `buttons` per player per tic. The full game [`TicCmd`] also carries
//! `chatchar`, but that field is not part of the LMP wire format.

use doom_types::TicCmd;

/// Size of a single demo tic command in the LMP wire format.
pub const DEMO_TIC_SIZE: usize = 4;

/// 4-byte per-player per-tic entry in the LMP stream.
///
/// Unlike the full [`TicCmd`], the LMP format stores only movement, the
/// quantized turn byte, and action buttons. Chat characters are not stored.
///
/// ```text
/// byte 0: forward_move (i8)
/// byte 1: side_move (i8)
/// byte 2: angle_turn (one byte, high 8 bits of the engine angle)
/// byte 3: buttons
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DemoTicCmd {
    /// Forward/backward movement (-128..127, positive = forward).
    pub forward_move: i8,
    /// Lateral strafe (-128..127, positive = right).
    pub side_move: i8,
    /// Quantized angle delta byte.
    pub angle_turn: i8,
    /// Button bitfield preserved in the demo stream.
    pub buttons: u8,
}

impl DemoTicCmd {
    /// Serialize this command to its 4-byte wire representation.
    #[must_use]
    pub const fn to_bytes(&self) -> [u8; DEMO_TIC_SIZE] {
        [
            self.forward_move as u8,
            self.side_move as u8,
            self.angle_turn as u8,
            self.buttons,
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
            angle_turn: data[2].cast_signed(),
            buttons: data[3],
        })
    }

    /// Convert a full [`TicCmd`] to a [`DemoTicCmd`].
    ///
    /// This is lossy by design: only the high byte of `angle_turn` is
    /// retained, so the lower 8 bits are dropped when serializing to LMP.
    /// `buttons` are preserved, and `chatchar` is omitted from the demo
    /// stream.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_demo::DemoTicCmd;
    /// use doom_types::TicCmd;
    ///
    /// let mut tic = TicCmd::default();
    /// tic.forward_move = 10;
    /// tic.angle_turn = 0x1234; // 16-bit angle
    ///
    /// let demo_cmd = DemoTicCmd::from_ticcmd(&tic);
    /// assert_eq!(demo_cmd.forward_move, 10);
    /// assert_eq!(demo_cmd.angle_turn, 0x12); // Only high byte kept
    /// ```
    #[must_use]
    pub const fn from_ticcmd(cmd: &TicCmd) -> Self {
        Self {
            forward_move: cmd.forward_move,
            side_move: cmd.side_move,
            angle_turn: (cmd.angle_turn >> 8) as i8,
            buttons: cmd.buttons,
        }
    }

    /// Convert this [`DemoTicCmd`] to a full [`TicCmd`].
    ///
    /// The demo turn byte is expanded back into engine units by shifting it
    /// into the high byte. The lower 8 bits are zeroed by design. `buttons`
    /// are preserved; `chatchar` and other padding fields default to zero.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_demo::DemoTicCmd;
    ///
    /// let demo_cmd = DemoTicCmd {
    ///     forward_move: 20,
    ///     side_move: -5,
    ///     angle_turn: 0x12,
    ///     buttons: 0x01, // BT_ATTACK
    /// };
    ///
    /// let tic = demo_cmd.to_ticcmd();
    /// assert_eq!(tic.forward_move, 20);
    /// assert_eq!(tic.angle_turn, 0x1200); // Shifted to high byte
    /// ```
    #[must_use]
    pub fn to_ticcmd(&self) -> TicCmd {
        TicCmd {
            forward_move: self.forward_move,
            side_move: self.side_move,
            angle_turn: i16::from(self.angle_turn) << 8,
            buttons: self.buttons,
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
    use doom_types::{TicCmd, bt};

    #[test]
    fn to_bytes_produces_4_bytes() {
        let cmd = DemoTicCmd {
            forward_move: 50,
            side_move: -10,
            angle_turn: 3,
            buttons: 0x1f,
        };
        let bytes = cmd.to_bytes();
        assert_eq!(bytes.len(), DEMO_TIC_SIZE);
    }

    #[test]
    fn from_bytes_roundtrip() {
        let original = DemoTicCmd {
            forward_move: 50,
            side_move: -10,
            angle_turn: 3,
            buttons: 0x1f,
        };
        let bytes = original.to_bytes();
        let parsed = DemoTicCmd::from_bytes(&bytes).expect("parse should succeed");
        assert_eq!(original, parsed);
    }

    #[test]
    fn to_ticcmd_preserves_buttons_and_zeroes_chat() {
        let demo_cmd = DemoTicCmd {
            forward_move: 25,
            side_move: -5,
            angle_turn: 1,
            buttons: bt::BT_ATTACK | bt::BT_USE,
        };
        let tic = demo_cmd.to_ticcmd();
        assert_eq!(tic.forward_move, 25);
        assert_eq!(tic.side_move, -5);
        assert_eq!(tic.angle_turn, 256);
        assert_eq!(tic.buttons, bt::BT_ATTACK | bt::BT_USE);
        assert_eq!(tic.chatchar, 0);
    }

    #[test]
    fn from_ticcmd_quantizes_angle_and_preserves_buttons() {
        let tic = TicCmd {
            forward_move: 30,
            side_move: -20,
            angle_turn: 0x1234,
            buttons: bt::BT_ATTACK | bt::BT_USE | bt::BT_CHANGE,
            chatchar: b'A',
            ..Default::default()
        };

        let demo_cmd = DemoTicCmd::from_ticcmd(&tic);
        assert_eq!(demo_cmd.forward_move, 30);
        assert_eq!(demo_cmd.side_move, -20);
        assert_eq!(demo_cmd.angle_turn, 0x12);
        assert_eq!(demo_cmd.buttons, bt::BT_ATTACK | bt::BT_USE | bt::BT_CHANGE);
    }

    #[test]
    fn negative_angle_turn_survives_roundtrip() {
        let original = DemoTicCmd {
            forward_move: 0,
            side_move: 0,
            angle_turn: -12,
            buttons: 0,
        };
        let bytes = original.to_bytes();
        let parsed = DemoTicCmd::from_bytes(&bytes).expect("Expected successful result in test");
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
            angle_turn: i8::MIN,
            buttons: u8::MAX,
        };
        let bytes = original.to_bytes();
        let parsed = DemoTicCmd::from_bytes(&bytes).expect("Expected successful result in test");
        assert_eq!(original, parsed);
    }

    #[test]
    fn default_is_all_zeros() {
        let cmd = DemoTicCmd::default();
        assert_eq!(cmd.forward_move, 0);
        assert_eq!(cmd.side_move, 0);
        assert_eq!(cmd.angle_turn, 0);
        assert_eq!(cmd.buttons, 0);
    }

    #[test]
    fn ticcmd_roundtrip_preserves_movement_and_quantized_angle() {
        let tic = TicCmd {
            forward_move: -50,
            side_move: 40,
            angle_turn: -8000,
            buttons: bt::BT_ATTACK | bt::BT_USE,
            ..Default::default()
        };

        let demo = DemoTicCmd::from_ticcmd(&tic);
        let back = demo.to_ticcmd();

        assert_eq!(back.forward_move, -50);
        assert_eq!(back.side_move, 40);
        assert_eq!(back.angle_turn, (tic.angle_turn >> 8) << 8);
        assert_eq!(back.buttons, bt::BT_ATTACK | bt::BT_USE);
    }

    #[test]
    fn vanilla_bytes_preserve_action_buttons_and_quantize_turn() {
        let tic = TicCmd {
            forward_move: 10,
            side_move: -5,
            angle_turn: 0x1200,
            buttons: bt::BT_ATTACK | bt::BT_USE | bt::BT_CHANGE | (3u8 << 3),
            chatchar: b'Z',
            ..Default::default()
        };

        let demo = DemoTicCmd::from_ticcmd(&tic);
        assert_eq!(
            demo.to_bytes(),
            [
                10u8,
                251u8,
                0x12,
                bt::BT_ATTACK | bt::BT_USE | bt::BT_CHANGE | (3u8 << 3)
            ]
        );

        let back = demo.to_ticcmd();
        assert_eq!(back.forward_move, 10);
        assert_eq!(back.side_move, -5);
        assert_eq!(back.angle_turn, 0x1200);
        assert_eq!(
            back.buttons,
            bt::BT_ATTACK | bt::BT_USE | bt::BT_CHANGE | (3u8 << 3)
        );
        assert_eq!(back.chatchar, 0);
    }
}
