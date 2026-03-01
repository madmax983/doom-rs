//! Networked game client for doom-net.
//!
//! [`NetClient`] wraps a [`UdpTransport`], maintains an [`InputLog`] for
//! rollback, and talks to a [`crate::server::RelayServer`].
//!
//! # Typical usage
//! 1. Call [`NetClient::connect`] to bind a local socket.
//! 2. Each tic: call [`NetClient::send_input`] with the local player's [`TicCmd`].
//! 3. Poll [`NetClient::poll_authoritative`] to receive the server's authoritative inputs.
//! 4. Call [`NetClient::save_snapshot`] / [`NetClient::restore_snapshot`] for rollback.

use std::net::SocketAddr;

use doom_game::{GameState, TicCmd};
use tokio::time::{Duration, timeout};

use crate::{
    NetError,
    input_log::InputLog,
    packet::{MAX_PLAYERS, TicPacket, WireTicCmd},
    rollback::SnapshotRing,
    transport::UdpTransport,
};

// ---------------------------------------------------------------------------
// NetClient
// ---------------------------------------------------------------------------

/// A networked game client that sends player input to a [`crate::server::RelayServer`]
/// and receives authoritative tic packets in return.
pub struct NetClient {
    transport: UdpTransport,
    server_addr: SocketAddr,
    /// This client's player slot (0-based).
    pub player_index: u8,
    /// The tic that will be sent on the next call to [`NetClient::send_input`].
    pub current_tic: u32,
    input_log: InputLog,
    snapshot_ring: SnapshotRing,
    /// Last authoritative tic received from the server.
    pub last_auth_tic: u32,
}

impl NetClient {
    /// Bind a local UDP socket at `local_addr` and prepare to talk to
    /// `server_addr` as player `player_index`.
    ///
    /// The server does not need to be reachable at construction time — the
    /// local bind is all that is required.
    ///
    /// # Errors
    /// Returns [`NetError::Network`] if the socket bind fails.
    pub async fn connect(
        local_addr: SocketAddr,
        server_addr: SocketAddr,
        player_index: u8,
    ) -> Result<Self, NetError> {
        let transport = UdpTransport::bind(local_addr).await?;
        Ok(Self {
            transport,
            server_addr,
            player_index,
            current_tic: 0,
            input_log: InputLog::new(),
            snapshot_ring: SnapshotRing::new(),
            last_auth_tic: 0,
        })
    }

    /// Encode `cmd` and send it to the server for [`Self::current_tic`].
    ///
    /// Records the command in the local [`InputLog`] and increments
    /// [`Self::current_tic`] afterwards (wrapping on overflow).
    ///
    /// # Errors
    /// Returns [`NetError::Network`] if the send fails.
    pub async fn send_input(&mut self, cmd: TicCmd) -> Result<(), NetError> {
        let wire_cmd = WireTicCmd::from(cmd);
        let player = self.player_index as usize;
        self.input_log.store(player, self.current_tic, wire_cmd);

        let mut cmds = [WireTicCmd::default(); MAX_PLAYERS];
        cmds[player % MAX_PLAYERS] = wire_cmd;

        let packet = TicPacket {
            tic:            self.current_tic,
            cmds,
            state_checksum: 0,
            sender:         self.player_index,
            ack_tic:        self.last_auth_tic,
        };

        self.transport
            .send_packet(&packet, self.server_addr)
            .await?;

        self.current_tic = self.current_tic.wrapping_add(1);
        Ok(())
    }

    /// Non-blocking poll for an authoritative packet from the server.
    ///
    /// Times out after 1 ms (suitable for per-frame polling without stalling
    /// the game loop).  On success, updates [`Self::last_auth_tic`] and
    /// stores all player inputs from the packet into the [`InputLog`].
    ///
    /// Returns `Some((tic, cmds))` on success, `None` on timeout or error.
    pub async fn poll_authoritative(&mut self) -> Option<(u32, [WireTicCmd; MAX_PLAYERS])> {
        match timeout(Duration::from_millis(1), self.transport.recv_packet()).await {
            Ok(Ok((packet, _addr))) => {
                self.last_auth_tic = packet.tic;
                for (i, cmd) in packet.cmds.iter().enumerate() {
                    self.input_log.store(i, packet.tic, *cmd);
                }
                Some((packet.tic, packet.cmds))
            }
            _ => None,
        }
    }

    /// Save a snapshot of `state` at `tic` for rollback.
    pub fn save_snapshot(&mut self, tic: u32, state: &GameState) {
        self.snapshot_ring.save(tic, state);
    }

    /// Retrieve the [`GameState`] snapshot for `tic`, if still in the ring.
    ///
    /// Returns a clone so the caller owns the value.
    #[must_use]
    pub fn restore_snapshot(&self, tic: u32) -> Option<GameState> {
        self.snapshot_ring.restore(tic).cloned()
    }

    /// Returns the local address this client's socket is bound to.
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

    /// `NetClient::connect` must succeed even when the server address is not
    /// reachable — only the local bind matters.
    #[tokio::test]
    async fn client_connect_succeeds() {
        let local: SocketAddr = "127.0.0.1:0".parse().expect("valid addr");
        // Use a plausible server address; no server needs to be running.
        let server: SocketAddr = "127.0.0.1:5555".parse().expect("valid addr");

        let result = NetClient::connect(local, server, 0).await;
        assert!(result.is_ok(), "NetClient::connect must return Ok");
    }

    /// After a successful `connect`, `local_addr()` must return `Ok` with a
    /// non-zero port.
    #[tokio::test]
    async fn client_local_addr_ok() {
        let local: SocketAddr = "127.0.0.1:0".parse().expect("valid addr");
        let server: SocketAddr = "127.0.0.1:5555".parse().expect("valid addr");

        let client = NetClient::connect(local, server, 0)
            .await
            .expect("connect must succeed");

        let addr = client.local_addr().expect("local_addr must return Ok");
        assert_ne!(addr.port(), 0, "OS must assign a non-zero port");
    }
}
