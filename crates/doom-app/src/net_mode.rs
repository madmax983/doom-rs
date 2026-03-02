//! Network play modes: relay server and networked game client wrapper.
//!
//! # Status
//! The doom-net crate now provides synchronous rollback data structures
//! (SnapshotRing, InputLog, RollbackManager, TicPacket, CRC32 checksums)
//! but does **not** yet implement actual UDP socket transport.
//!
//! This module provides the conversion helpers between doom-tui's `TicInput`
//! and doom-game's `TicCmd`, plus the wire-format `doom_net::TicCmd`.
//! The `NetGameApp` wrapper and `run_server` will be implemented once
//! doom-net gains a real transport layer.

use doom_game::TicCmd;
use doom_tui::TicInput;

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/// Convert a [`TicInput`] (from doom-tui) to a [`TicCmd`] (for doom-game).
///
/// Only the wire-compatible fields are copied; console/UI fields are dropped.
pub(crate) fn ticinput_to_ticcmd(input: TicInput) -> TicCmd {
    let mut cmd = TicCmd::default();
    cmd.forward_move = input.forward_move;
    cmd.side_move = input.side_move;
    cmd.angle_turn = input.angle_turn;
    cmd.buttons = input.buttons;
    cmd.chatchar = input.chatchar;
    cmd
}

/// Convert a [`TicInput`] to the wire format [`doom_net::TicCmd`].
#[allow(dead_code)]
pub(crate) fn ticinput_to_wire(input: TicInput) -> doom_net::TicCmd {
    doom_net::TicCmd {
        forward_move: input.forward_move,
        side_move: input.side_move,
        angle_turn: input.angle_turn,
        buttons: input.buttons,
        chatchar: input.chatchar,
    }
}

/// Convert a [`doom_net::TicCmd`] to a [`TicCmd`] (doom-game format).
#[allow(dead_code)]
pub(crate) fn wire_to_ticcmd(w: doom_net::TicCmd) -> TicCmd {
    let mut cmd = TicCmd::default();
    cmd.forward_move = w.forward_move;
    cmd.side_move = w.side_move;
    cmd.angle_turn = w.angle_turn;
    cmd.buttons = w.buttons;
    cmd.chatchar = w.chatchar;
    cmd
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticinput_to_wire_preserves_forward_move() {
        let input = TicInput {
            forward_move: 42,
            ..Default::default()
        };
        let wire = ticinput_to_wire(input);
        assert_eq!(wire.forward_move, 42);
    }

    #[test]
    fn wire_to_ticcmd_preserves_buttons() {
        let wire = doom_net::TicCmd {
            buttons: 0b0101,
            ..Default::default()
        };
        let cmd = wire_to_ticcmd(wire);
        assert_eq!(cmd.buttons, 0b0101);
    }

    #[test]
    fn wire_roundtrip_is_identity() {
        let input = TicInput {
            forward_move: 50,
            side_move: -10,
            angle_turn: 1000,
            buttons: 3,
            chatchar: b'a',
            ..Default::default()
        };
        let wire = ticinput_to_wire(input);
        let cmd = wire_to_ticcmd(wire);
        assert_eq!(cmd.forward_move, 50);
        assert_eq!(cmd.side_move, -10);
        assert_eq!(cmd.angle_turn, 1000);
        assert_eq!(cmd.buttons, 3);
    }

    #[test]
    fn ticinput_to_ticcmd_copies_all_fields() {
        let input = TicInput {
            forward_move: 100,
            side_move: -50,
            angle_turn: 640,
            buttons: 0x03,
            chatchar: b'z',
            ..Default::default()
        };
        let cmd = ticinput_to_ticcmd(input);
        assert_eq!(cmd.forward_move, 100);
        assert_eq!(cmd.side_move, -50);
        assert_eq!(cmd.angle_turn, 640);
        assert_eq!(cmd.buttons, 0x03);
        assert_eq!(cmd.chatchar, b'z');
    }

    #[test]
    fn default_wire_cmd_is_all_zeros() {
        let wire = doom_net::TicCmd::default();
        assert_eq!(wire.forward_move, 0);
        assert_eq!(wire.side_move, 0);
        assert_eq!(wire.angle_turn, 0);
        assert_eq!(wire.buttons, 0);
        assert_eq!(wire.chatchar, 0);
    }

    #[test]
    fn wire_cmd_to_ticcmd_preserves_chatchar() {
        let wire = doom_net::TicCmd {
            chatchar: b'X',
            ..Default::default()
        };
        let cmd = wire_to_ticcmd(wire);
        assert_eq!(cmd.chatchar, b'X');
    }
}
