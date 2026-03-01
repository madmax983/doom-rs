//! Relay server for doom-net multiplayer.
//!
//! [`RelayServer`] accepts UDP packets from up to 4 clients, collects each
//! player's input for every tic, and broadcasts an authoritative [`TicPacket`]
//! to all registered clients once every player has submitted their input for
//! that tic.
//!
//! # Design
//! - Up to [`MAX_PLAYERS`] clients are supported.
//! - Client registration is implicit: the first packet from a `(player_index,
//!   addr)` pair registers that player.
//! - `pending` accumulates inputs until a tic is complete; then it is cleared
//!   and the authoritative packet is broadcast.

use std::collections::HashMap;
use std::net::SocketAddr;

use crate::{
    NetError,
    packet::{MAX_PLAYERS, TicPacket, WireTicCmd},
    transport::UdpTransport,
};

// ---------------------------------------------------------------------------
// RelayServer
// ---------------------------------------------------------------------------

/// A UDP relay server that synchronises player inputs across a session.
pub struct RelayServer {
    transport: UdpTransport,
    /// Maps player index → their client socket address.
    clients: HashMap<u8, SocketAddr>,
    /// Per-tic accumulated inputs: `tic → [Option<WireTicCmd>; MAX_PLAYERS]`.
    pending: HashMap<u32, [Option<WireTicCmd>; MAX_PLAYERS]>,
    /// Number of players in this session (clamped to [`MAX_PLAYERS`]).
    pub num_players: u8,
}

impl RelayServer {
    /// Bind to `addr` and prepare a session for `num_players` clients.
    ///
    /// `num_players` is silently clamped to [`MAX_PLAYERS`].
    ///
    /// # Errors
    /// Returns [`NetError::Network`] if the socket bind fails.
    pub async fn bind(addr: SocketAddr, num_players: u8) -> Result<Self, NetError> {
        let transport = UdpTransport::bind(addr).await?;
        Ok(Self {
            transport,
            clients: HashMap::new(),
            pending: HashMap::new(),
            num_players: num_players.min(MAX_PLAYERS as u8),
        })
    }

    /// Process one incoming packet.
    ///
    /// If this packet completes a tic (all `num_players` players have submitted),
    /// broadcasts the authoritative [`TicPacket`] to every registered client and
    /// returns `Some(tic)`.  Returns `None` if the tic is still incomplete.
    ///
    /// Individual broadcast send errors are silently ignored (UDP best-effort).
    ///
    /// # Errors
    /// Returns [`NetError`] if receiving the next packet fails (socket error or
    /// decode error).
    pub async fn step(&mut self) -> Result<Option<u32>, NetError> {
        let (packet, addr) = self.transport.recv_packet().await?;

        // Implicit client registration: first packet from a sender registers them.
        self.clients.entry(packet.sender).or_insert(addr);

        // Accumulate input for this tic.
        let entry = self
            .pending
            .entry(packet.tic)
            .or_insert([None; MAX_PLAYERS]);
        let sender_idx = packet.sender as usize % MAX_PLAYERS;
        entry[sender_idx] = Some(packet.cmds[sender_idx]);

        // Check whether every expected player has submitted for this tic.
        let num = self.num_players as usize;
        let complete = entry[..num].iter().all(|c| c.is_some());

        if complete {
            // Copy before we remove the entry.
            let cmds = *entry;
            self.pending.remove(&packet.tic);

            // Build the authoritative packet: sender=255 means "server".
            let auth = TicPacket {
                tic:            packet.tic,
                cmds:           cmds.map(|c| c.unwrap_or_default()),
                state_checksum: 0,
                sender:         255,
                ack_tic:        packet.tic,
            };

            // Broadcast to every registered client; ignore individual send errors.
            for client_addr in self.clients.values() {
                let _ = self.transport.send_packet(&auth, *client_addr).await;
            }

            Ok(Some(packet.tic))
        } else {
            Ok(None)
        }
    }

    /// Returns the local address the server socket is bound to.
    ///
    /// # Errors
    /// Returns [`NetError::Network`] if the OS query fails.
    pub fn local_addr(&self) -> Result<SocketAddr, NetError> {
        self.transport.local_addr()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        packet::{TicPacket, WireTicCmd, MAX_PLAYERS},
        transport::UdpTransport,
    };

    fn player_packet(tic: u32, player: u8, forward: i8) -> TicPacket {
        let mut cmds = [WireTicCmd::default(); MAX_PLAYERS];
        cmds[player as usize % MAX_PLAYERS] = WireTicCmd {
            forward_move: forward,
            ..WireTicCmd::default()
        };
        TicPacket {
            tic,
            cmds,
            state_checksum: 0,
            sender: player,
            ack_tic: 0,
        }
    }

    /// `RelayServer::bind` with a single player must succeed.
    #[tokio::test]
    async fn server_bind_succeeds() {
        let addr: SocketAddr = "127.0.0.1:0".parse().expect("valid addr");
        let server = RelayServer::bind(addr, 1).await;
        assert!(server.is_ok(), "RelayServer::bind must return Ok");
        let server = server.expect("already checked");
        let local = server.local_addr().expect("local_addr must return Ok");
        assert_ne!(local.port(), 0, "OS must assign a non-zero port");
    }

    /// A single-player session: after the client sends a packet for tic 0,
    /// `step()` must return `Some(0)` (tic complete + broadcast).
    #[tokio::test]
    async fn server_single_player_completes_tic() {
        let any: SocketAddr = "127.0.0.1:0".parse().expect("valid addr");

        let mut server = RelayServer::bind(any, 1)
            .await
            .expect("server bind");
        let server_addr = server.local_addr().expect("server local addr");

        // Client transport (player 0).
        let client_transport = UdpTransport::bind(any).await.expect("client bind");

        let pkt = player_packet(0, 0, 10);
        client_transport
            .send_packet(&pkt, server_addr)
            .await
            .expect("client send");

        let result = server.step().await.expect("step must not error");
        assert_eq!(
            result,
            Some(0),
            "single-player tic 0 must be complete after one packet"
        );
    }
}
