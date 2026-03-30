//! Wire protocol types for doom-net.
//!
//! [`TicPacket`] is the single datagram type exchanged between clients and the
//! server.  Serialisation is manual little-endian, no `serde` or `bincode` on
//! the hot path.
//!
//! [`TicCmd`] is the doom-net wire-format player input command, layout-compatible
//! with `doom_game::TicCmd`.  It is defined here so that doom-net only depends on
//! `doom-types`, not `doom-game`.

/// Maximum number of players in a multiplayer session.
pub const MAX_PLAYERS: usize = 4;

/// Maximum number of tics the rollback system can rewind.
pub const MAX_ROLLBACK_TICS: usize = 8;

// ---------------------------------------------------------------------------
// TicCmd -- wire-format player input
// ---------------------------------------------------------------------------

/// One tic of player input -- the wire-compatible command struct.
///
/// Layout mirrors `doom_game::TicCmd` field-for-field so that the two types
/// can be transmuted or field-copied at the doom-game/doom-net boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct TicCmd {
    /// Forward/backward movement (-128..127, positive = forward).
    pub forward_move: i8,
    /// Lateral strafe (-128..127, positive = right).
    pub side_move: i8,
    /// Angle delta in 16-bit BAM units (shifted left 16 -> 32-bit BAM).
    pub angle_turn: i16,
    /// Button bitfield (`bt::BT_*` flags).
    pub buttons: u8,
    /// ASCII chat character (0 = none).
    pub chatchar: u8,
}

// ---------------------------------------------------------------------------
// TicCmd wire size
// ---------------------------------------------------------------------------

/// Wire size of one [`TicCmd`] when serialized:
/// `i8 + i8 + i16 + u8 + u8` = 6 bytes.
const TICCMD_WIRE_SIZE: usize = 6;

/// Wire size of one [`TicPacket`]:
/// `u32 tic + u8 sender + u32 ack_tic + u32 state_checksum + 4 * 6 cmds`
/// = 4 + 1 + 4 + 4 + 24 = 37 bytes.
pub const TIC_PACKET_SIZE: usize = 4 + 1 + 4 + 4 + (TICCMD_WIRE_SIZE * MAX_PLAYERS);

// ---------------------------------------------------------------------------
// TicPacket
// ---------------------------------------------------------------------------

/// One network packet carrying inputs for one tic.
///
/// Wire layout (little-endian, 37 bytes):
/// ```text
/// [ tic: u32 ] [ sender: u8 ] [ ack_tic: u32 ] [ state_checksum: u32 ]
/// [ cmds[0]: 6B ] [ cmds[1]: 6B ] [ cmds[2]: 6B ] [ cmds[3]: 6B ]
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct TicPacket {
    /// The game tic this packet covers.
    pub tic: u32,
    /// Which player slot sent this packet (0-based, 255 = server).
    pub sender: u8,
    /// Last tic the sender has received from the remote (ACK flow control).
    pub ack_tic: u32,
    /// CRC32 of the sender's game state at the END of `tic - 1`.
    pub state_checksum: u32,
    /// Input commands -- one per player slot (unused slots = zeroed).
    pub cmds: [TicCmd; MAX_PLAYERS],
}

impl TicPacket {
    /// Serialize this packet to a `Vec<u8>` in little-endian format.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_net::{TicPacket, TicCmd, packet::MAX_PLAYERS};
    ///
    /// let packet = TicPacket {
    ///     tic: 42,
    ///     sender: 0,
    ///     ack_tic: 40,
    ///     state_checksum: 12345,
    ///     cmds: [TicCmd::default(); MAX_PLAYERS],
    /// };
    ///
    /// let bytes = packet.to_bytes();
    /// assert_eq!(bytes.len(), doom_net::packet::TIC_PACKET_SIZE);
    /// ```
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(TIC_PACKET_SIZE);

        buf.extend_from_slice(&self.tic.to_le_bytes());
        buf.push(self.sender);
        buf.extend_from_slice(&self.ack_tic.to_le_bytes());
        buf.extend_from_slice(&self.state_checksum.to_le_bytes());

        for cmd in &self.cmds {
            buf.push(cmd.forward_move as u8);
            buf.push(cmd.side_move as u8);
            buf.extend_from_slice(&cmd.angle_turn.to_le_bytes());
            buf.push(cmd.buttons);
            buf.push(cmd.chatchar);
        }

        buf
    }

    /// Deserialize a packet from a little-endian byte slice.
    ///
    /// Returns `None` if `data` is too short.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_net::{TicPacket, TicCmd, packet::MAX_PLAYERS};
    ///
    /// let original = TicPacket {
    ///     tic: 100,
    ///     sender: 1,
    ///     ack_tic: 99,
    ///     state_checksum: 9876,
    ///     cmds: [TicCmd::default(); MAX_PLAYERS],
    /// };
    ///
    /// let bytes = original.to_bytes();
    /// let parsed = TicPacket::from_bytes(&bytes).expect("Should parse successfully");
    ///
    /// assert_eq!(parsed.tic, 100);
    /// assert_eq!(parsed.sender, 1);
    /// ```
    #[must_use]
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < TIC_PACKET_SIZE {
            return None;
        }

        let mut offset = 0;

        let tic = u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?);
        offset += 4;

        let sender = data[offset];
        offset += 1;

        let ack_tic = u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?);
        offset += 4;

        let state_checksum = u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?);
        offset += 4;

        let mut cmds = [TicCmd::default(); MAX_PLAYERS];
        for cmd in &mut cmds {
            cmd.forward_move = data[offset].cast_signed();
            offset += 1;
            cmd.side_move = data[offset].cast_signed();
            offset += 1;
            cmd.angle_turn = i16::from_le_bytes(data[offset..offset + 2].try_into().ok()?);
            offset += 2;
            cmd.buttons = data[offset];
            offset += 1;
            cmd.chatchar = data[offset];
            offset += 1;
        }

        Some(Self {
            tic,
            sender,
            ack_tic,
            state_checksum,
            cmds,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_packet() -> TicPacket {
        let mut cmds = [TicCmd::default(); MAX_PLAYERS];
        cmds[0] = TicCmd {
            forward_move: 50,
            side_move: -10,
            angle_turn: 640,
            buttons: 0x01,
            chatchar: 0,
        };
        cmds[1] = TicCmd {
            forward_move: -20,
            side_move: 30,
            angle_turn: -512,
            buttons: 0x03,
            chatchar: b'Z',
        };
        TicPacket {
            tic: 42,
            sender: 0,
            ack_tic: 41,
            state_checksum: 0xDEAD_BEEF,
            cmds,
        }
    }

    #[test]
    fn to_bytes_produces_correct_size() {
        let pkt = sample_packet();
        let bytes = pkt.to_bytes();
        assert_eq!(
            bytes.len(),
            TIC_PACKET_SIZE,
            "to_bytes must produce exactly TIC_PACKET_SIZE bytes"
        );
    }

    #[test]
    fn roundtrip_to_bytes_from_bytes() {
        let original = sample_packet();
        let bytes = original.to_bytes();
        let decoded = TicPacket::from_bytes(&bytes).expect("from_bytes must succeed");
        assert_eq!(original, decoded, "roundtrip must preserve packet");
    }

    #[test]
    fn from_bytes_too_short_returns_none() {
        let short = vec![0u8; TIC_PACKET_SIZE - 1];
        assert!(
            TicPacket::from_bytes(&short).is_none(),
            "from_bytes with short data must return None"
        );
    }

    #[test]
    fn from_bytes_exact_size_works() {
        let pkt = sample_packet();
        let bytes = pkt.to_bytes();
        assert_eq!(bytes.len(), TIC_PACKET_SIZE);
        let decoded = TicPacket::from_bytes(&bytes);
        assert!(
            decoded.is_some(),
            "from_bytes with exact size must return Some"
        );
    }

    #[test]
    fn packet_preserves_sender() {
        let mut pkt = sample_packet();
        pkt.sender = 3;
        let bytes = pkt.to_bytes();
        let decoded = TicPacket::from_bytes(&bytes).expect("decode");
        assert_eq!(decoded.sender, 3, "sender field must survive roundtrip");
    }

    #[test]
    fn packet_preserves_tic_number() {
        let mut pkt = sample_packet();
        pkt.tic = 0xFFFF_FFFE;
        let bytes = pkt.to_bytes();
        let decoded = TicPacket::from_bytes(&bytes).expect("decode");
        assert_eq!(decoded.tic, 0xFFFF_FFFE, "tic field must survive roundtrip");
    }

    #[test]
    fn packet_preserves_all_four_cmds() {
        let pkt = sample_packet();
        let bytes = pkt.to_bytes();
        let decoded = TicPacket::from_bytes(&bytes).expect("decode");
        for i in 0..MAX_PLAYERS {
            assert_eq!(
                decoded.cmds[i].forward_move, pkt.cmds[i].forward_move,
                "cmd[{i}].forward_move mismatch"
            );
            assert_eq!(
                decoded.cmds[i].side_move, pkt.cmds[i].side_move,
                "cmd[{i}].side_move mismatch"
            );
            assert_eq!(
                decoded.cmds[i].angle_turn, pkt.cmds[i].angle_turn,
                "cmd[{i}].angle_turn mismatch"
            );
            assert_eq!(
                decoded.cmds[i].buttons, pkt.cmds[i].buttons,
                "cmd[{i}].buttons mismatch"
            );
            assert_eq!(
                decoded.cmds[i].chatchar, pkt.cmds[i].chatchar,
                "cmd[{i}].chatchar mismatch"
            );
        }
    }

    #[test]
    fn from_bytes_with_extra_trailing_data_still_works() {
        let pkt = sample_packet();
        let mut bytes = pkt.to_bytes();
        bytes.extend_from_slice(&[0xFF; 16]); // extra garbage at end
        let decoded = TicPacket::from_bytes(&bytes).expect("decode with trailing data");
        assert_eq!(decoded, pkt, "trailing data must be ignored");
    }

    #[test]
    fn packet_preserves_state_checksum() {
        let pkt = sample_packet();
        let bytes = pkt.to_bytes();
        let decoded = TicPacket::from_bytes(&bytes).expect("decode");
        assert_eq!(
            decoded.state_checksum, 0xDEAD_BEEF,
            "state_checksum must survive roundtrip"
        );
    }

    #[test]
    fn packet_preserves_ack_tic() {
        let pkt = sample_packet();
        let bytes = pkt.to_bytes();
        let decoded = TicPacket::from_bytes(&bytes).expect("decode");
        assert_eq!(decoded.ack_tic, 41, "ack_tic must survive roundtrip");
    }

    #[test]
    fn empty_bytes_returns_none() {
        assert!(TicPacket::from_bytes(&[]).is_none());
    }
}
