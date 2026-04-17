//! Wire protocol types for doom-net.
//!
//! [`TicPacket`] is the single datagram type exchanged between clients and the
//! server.  Serialisation is manual little-endian, no `serde` or `bincode` on
//! the hot path.
//!
//! [`` `` `` `TicCmd` `` `` ``] is the wire-format player input command. It is imported from `doom-types`
//! so that it can be shared across the entire workspace.

use doom_types::TicCmd;

/// Maximum number of players in a multiplayer session.
pub const MAX_PLAYERS: usize = 4;

/// Maximum number of tics the rollback system can rewind.
pub const MAX_ROLLBACK_TICS: usize = 8;

// ---------------------------------------------------------------------------
// TicCmd wire size
// ---------------------------------------------------------------------------

/// Wire size of one [`` `` `` `TicCmd` `` `` ``] when serialized:
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
    /// use doom_net::{TicPacket, packet::MAX_PLAYERS};
use doom_types::TicCmd;
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
    /// use doom_net::{TicPacket, packet::MAX_PLAYERS};
use doom_types::TicCmd;
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
        cmds[0].forward_move = 50;
        cmds[0].side_move = -10;
        cmds[0].angle_turn = 640;
        cmds[0].buttons = 0x01;
        cmds[0].chatchar = 0;
        cmds[1].forward_move = -20;
        cmds[1].side_move = 30;
        cmds[1].angle_turn = -512;
        cmds[1].buttons = 0x03;
        cmds[1].chatchar = b'Z';

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

    #[test]
    fn from_bytes_rejects_missing_data_at_cmd_boundary() {
        let pkt = sample_packet();
        let mut bytes = pkt.to_bytes();

        // Truncate halfway through the first cmd (37 bytes is full size)
        // Let's truncate to 35 bytes
        bytes.truncate(TIC_PACKET_SIZE - 2);

        assert!(TicPacket::from_bytes(&bytes).is_none());
    }
}
