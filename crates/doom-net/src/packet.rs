//! Wire format: `TicPacket` (bincode), CRC32 desync detection.
//!
//! `TicCmd` in doom-game is `repr(C)` without `bincode::Encode`/`Decode`.
//! We define `WireTicCmd` as the canonical wire representation and provide
//! `From` impls to convert to/from `doom_game::TicCmd`.
//!
//! # Bincode version
//! Uses bincode 2.x with `bincode::config::standard()`.
//! Encode: `bincode::encode_to_vec(&value, config)`
//! Decode: `bincode::decode_from_slice::<T, _>(bytes, config)`

use crate::NetError;

/// Maximum number of players in a multiplayer session.
pub const MAX_PLAYERS: usize = 4;

// ---------------------------------------------------------------------------
// WireTicCmd — wire-format player input
// ---------------------------------------------------------------------------

/// Wire-format player input command.
///
/// Mirrors `doom_game::TicCmd` field-for-field (omitting the private `_pad`).
/// Derives `bincode::Encode`/`Decode` for deterministic serialization and
/// `serde::Serialize`/`Deserialize` for interop.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    bincode::Encode,
    bincode::Decode,
)]
pub struct WireTicCmd {
    /// Forward/backward movement (-128..127, positive = forward).
    pub forward_move: i8,
    /// Lateral strafe (-128..127, positive = right).
    pub side_move: i8,
    /// Angle delta in 16-bit BAM units.
    pub angle_turn: i16,
    /// Button bitfield (`bt::BT_*` flags).
    pub buttons: u8,
    /// ASCII chat character (0 = none).
    pub chatchar: u8,
}

impl From<doom_game::TicCmd> for WireTicCmd {
    fn from(cmd: doom_game::TicCmd) -> Self {
        Self {
            forward_move: cmd.forward_move,
            side_move:    cmd.side_move,
            angle_turn:   cmd.angle_turn,
            buttons:      cmd.buttons,
            chatchar:     cmd.chatchar,
        }
    }
}

impl From<WireTicCmd> for doom_game::TicCmd {
    fn from(w: WireTicCmd) -> Self {
        // `TicCmd::default()` zeroes all fields including the private `_pad`.
        // We then overwrite only the public wire fields.
        let mut cmd = doom_game::TicCmd::default();
        cmd.forward_move = w.forward_move;
        cmd.side_move    = w.side_move;
        cmd.angle_turn   = w.angle_turn;
        cmd.buttons      = w.buttons;
        cmd.chatchar     = w.chatchar;
        cmd
    }
}

// ---------------------------------------------------------------------------
// TicPacket — one UDP datagram
// ---------------------------------------------------------------------------

/// One network packet carrying inputs for one tic from one player.
///
/// Encoded with bincode 2 `standard()` config for deterministic wire bytes.
#[derive(Debug, Clone, PartialEq, bincode::Encode, bincode::Decode)]
pub struct TicPacket {
    /// The game tic this packet covers.
    pub tic: u32,
    /// Input commands — one per player slot (unused slots = zeroed `WireTicCmd`).
    pub cmds: [WireTicCmd; MAX_PLAYERS],
    /// CRC32 of the sender's `GameState` at the END of `tic - 1`.
    ///
    /// Used for desync detection.  `0` = not available.
    pub state_checksum: u32,
    /// Which player slot sent this packet (0-based).
    pub sender: u8,
    /// Last tic the sender has received from the server (ACK flow control).
    pub ack_tic: u32,
}

impl TicPacket {
    /// Encode the packet to bytes using bincode 2 `standard()`.
    ///
    /// Returns an empty `Vec` on encode failure (should not happen for valid
    /// data — the type invariants guarantee encodability).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        // SAFETY-NOTE: encode_to_vec only fails if the type contains
        // unencodable values (e.g. unsupported lengths). WireTicCmd and all
        // primitive fields here are always encodable; the unwrap_or_default
        // is the documented fallback as per quality standards.
        bincode::encode_to_vec(self, bincode::config::standard()).unwrap_or_default()
    }

    /// Decode a packet from bytes.
    ///
    /// Returns `Err(NetError::Decode)` if the bytes are malformed or empty.
    pub fn decode(data: &[u8]) -> Result<Self, NetError> {
        if data.is_empty() {
            return Err(NetError::Decode("empty packet".to_string()));
        }
        bincode::decode_from_slice(data, bincode::config::standard())
            .map(|(packet, _consumed)| packet)
            .map_err(|e| NetError::Decode(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_packet() -> TicPacket {
        let mut cmds = [WireTicCmd::default(); MAX_PLAYERS];
        cmds[0] = WireTicCmd {
            forward_move: 50,
            side_move:    -10,
            angle_turn:   640,
            buttons:      0x01,
            chatchar:     0,
        };
        TicPacket {
            tic:            42,
            cmds,
            state_checksum: 0xDEAD_BEEF,
            sender:         0,
            ack_tic:        41,
        }
    }

    #[test]
    fn packet_encode_decode_roundtrip() {
        let original = sample_packet();
        let bytes    = original.encode();
        let decoded  = TicPacket::decode(&bytes).expect("decode must succeed");
        assert_eq!(original, decoded);
    }

    #[test]
    fn packet_encode_is_deterministic() {
        let packet = sample_packet();
        let a = packet.encode();
        let b = packet.encode();
        assert_eq!(a, b, "two encodes of the same packet must be byte-identical");
    }

    #[test]
    fn wire_ticcmd_from_game_ticcmd() {
        let mut game_cmd = doom_game::TicCmd::default();
        game_cmd.forward_move = 100;
        game_cmd.side_move    = -50;
        game_cmd.angle_turn   = 1024;
        game_cmd.buttons      = 0x03;
        game_cmd.chatchar     = b'A';
        let wire: WireTicCmd = game_cmd.into();
        assert_eq!(wire.forward_move, 100);
        assert_eq!(wire.side_move,    -50);
        assert_eq!(wire.angle_turn,   1024);
        assert_eq!(wire.buttons,      0x03);
        assert_eq!(wire.chatchar,     b'A');

        let back: doom_game::TicCmd = wire.into();
        assert_eq!(back.forward_move, game_cmd.forward_move);
        assert_eq!(back.side_move,    game_cmd.side_move);
        assert_eq!(back.angle_turn,   game_cmd.angle_turn);
        assert_eq!(back.buttons,      game_cmd.buttons);
        assert_eq!(back.chatchar,     game_cmd.chatchar);
    }

    #[test]
    fn empty_packet_decodes_to_err() {
        let result = TicPacket::decode(&[]);
        assert!(result.is_err(), "decoding empty bytes must return Err, not panic");
    }
}
