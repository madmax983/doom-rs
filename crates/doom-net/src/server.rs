//! Relay server for doom-net multiplayer.
//!
//! [`RelayServer`] accepts connections from up to [`MAX_PLAYERS`] clients,
//! assigns player slots, and relays [`TicPacket`]s between connected peers.
//! The server itself does not run the game simulation -- it only forwards
//! authoritative input.

use std::io;
use std::net::SocketAddr;
use std::time::Instant;

use crate::packet::{MAX_PLAYERS, TicPacket};
use crate::transport::{
    ConnectionState, NetConfig, NetTransport, is_join_request, make_join_response,
};

// ---------------------------------------------------------------------------
// ServerSlot
// ---------------------------------------------------------------------------

/// One player slot on the relay server.
#[derive(Debug, Clone)]
pub struct ServerSlot {
    /// The remote address of this player's client.
    pub addr: SocketAddr,
    /// The player number (0-based).
    pub player_num: u8,
    /// The last tic number received from this player.
    pub last_tic: u32,
    /// Timestamp of the last packet received from this player.
    pub last_heard: Instant,
    /// Whether this slot is currently connected.
    pub connected: bool,
}

// ---------------------------------------------------------------------------
// RelayServer
// ---------------------------------------------------------------------------

/// A simple relay server that forwards [`TicPacket`]s between connected
/// clients.
///
/// Slots are assigned on a first-come-first-served basis up to
/// `config.max_players` (capped at [`MAX_PLAYERS`]).
#[derive(Debug)]
pub struct RelayServer {
    /// The underlying UDP transport.
    transport: NetTransport,
    /// Player slots (up to [`MAX_PLAYERS`]).
    slots: [Option<ServerSlot>; MAX_PLAYERS],
    /// Server configuration.
    config: NetConfig,
}

impl RelayServer {
    /// Create a new relay server, binding on `0.0.0.0:{config.port}`.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if binding to the port fails.
    pub fn new(config: NetConfig) -> io::Result<Self> {
        let addr = format!("0.0.0.0:{}", config.port);
        let transport = NetTransport::bind(&addr)?;
        Ok(Self {
            transport,
            slots: [const { None }; MAX_PLAYERS],
            config,
        })
    }

    /// Create a relay server bound to a specific address (useful for tests
    /// with `"127.0.0.1:0"`).
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if binding to the specific address fails.
    pub fn bind(addr: &str, config: NetConfig) -> io::Result<Self> {
        let transport = NetTransport::bind(addr)?;
        Ok(Self {
            transport,
            slots: [const { None }; MAX_PLAYERS],
            config,
        })
    }

    /// Accept a connection from `addr`, assigning the first available slot.
    ///
    /// Returns `Some(slot_number)` on success, or `None` if the server is full.
    /// If the address is already connected, returns the existing slot.
    pub fn accept_connection(&mut self, addr: SocketAddr) -> Option<u8> {
        // Check if this address is already connected.
        for s in self.slots.iter().flatten() {
            if s.addr == addr && s.connected {
                return Some(s.player_num);
            }
        }

        // Find the first empty slot within max_players.
        let max = (self.config.max_players as usize).min(MAX_PLAYERS);
        for i in 0..max {
            if self.slots[i].is_none() {
                self.slots[i] = Some(ServerSlot {
                    addr,
                    player_num: i as u8,
                    last_tic: 0,
                    last_heard: Instant::now(),
                    connected: true,
                });
                return Some(i as u8);
            }
        }

        None // server full
    }

    /// Disconnect a player by slot number.
    pub const fn disconnect_player(&mut self, slot: u8) {
        let idx = slot as usize;
        if idx < MAX_PLAYERS {
            self.slots[idx] = None;
        }
    }

    /// Broadcast a packet to all connected slots, optionally excluding the
    /// sender's slot.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if sending the packet via the underlying transport fails.
    pub fn broadcast_packet(
        &mut self,
        packet: &TicPacket,
        exclude_sender: Option<u8>,
    ) -> io::Result<()> {
        let data = packet.to_bytes();
        #[allow(clippy::manual_flatten)]
        for s in self.slots.iter().flatten() {
            if s.connected {
                // Skip the sender if requested.
                if exclude_sender == Some(s.player_num) {
                    continue;
                }
                // Best-effort send -- UDP may silently drop.
                let _ = self.transport.send_raw(&data, &s.addr);
            }
        }
        Ok(())
    }

    /// Count of currently connected players.
    #[must_use]
    pub fn connected_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| s.as_ref().is_some_and(|slot| slot.connected))
            .count()
    }

    /// Non-blocking poll: receive one packet and identify the sender by
    /// address.
    ///
    /// If the packet is a join request, the connection is accepted and a
    /// response is sent automatically.
    ///
    /// Returns `Ok(Some((packet, slot)))` for a gameplay packet, or
    /// `Ok(None)` if nothing was received (or a handshake was handled
    /// internally).
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if reading from the socket fails.
    pub fn poll_once(&mut self) -> io::Result<Option<(TicPacket, u8)>> {
        let recv = self.transport.recv_packet()?;
        let Some((pkt, addr)) = recv else {
            return Ok(None);
        };

        // Handle join requests internally.
        if is_join_request(&pkt) {
            if let Some(slot) = self.accept_connection(addr) {
                let response = make_join_response(slot);
                let data = response.to_bytes();
                let _ = self.transport.send_raw(&data, &addr);
            }
            return Ok(None);
        }

        // Identify the sender by address.
        let sender_slot = self.slot_for_addr(&addr);
        if let Some(slot_num) = sender_slot {
            // Update last_heard and last_tic.
            if let Some(ref mut s) = self.slots[slot_num as usize] {
                s.last_heard = Instant::now();
                if pkt.tic > s.last_tic {
                    s.last_tic = pkt.tic;
                }
            }
            return Ok(Some((pkt, slot_num)));
        }

        // Unknown sender -- drop.
        Ok(None)
    }

    /// Disconnect any slots that have not been heard from within
    /// `config.timeout_ms`.
    pub fn check_timeouts(&mut self) {
        let timeout = self.config.timeout_ms;
        for slot in &mut self.slots {
            let timed_out = slot.as_ref().is_some_and(|s| {
                s.connected && s.last_heard.elapsed().as_millis() as u64 >= timeout
            });
            if timed_out {
                *slot = None;
            }
        }
    }

    /// Look up which slot number owns `addr`, if any.
    fn slot_for_addr(&self, addr: &SocketAddr) -> Option<u8> {
        for s in self.slots.iter().flatten() {
            if s.addr == *addr && s.connected {
                return Some(s.player_num);
            }
        }
        None
    }

    /// The local address the server is bound to (useful for tests).
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if the socket address cannot be retrieved.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.transport.local_addr()
    }

    /// Borrow the transport (for stats, etc.).
    #[must_use]
    pub const fn transport(&self) -> &NetTransport {
        &self.transport
    }

    /// Set the connection state on the inner transport.
    pub const fn set_transport_state(&mut self, state: ConnectionState) {
        self.transport.set_state(state);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> NetConfig {
        NetConfig {
            port: 0, // OS-assigned
            max_players: 4,
            timeout_ms: 100, // short for tests
            keepalive_interval_ms: 50,
        }
    }

    #[test]
    fn relay_server_new_binds_successfully() {
        let server = RelayServer::bind("127.0.0.1:0", test_config());
        assert!(
            server.is_ok(),
            "RelayServer::bind must succeed on 127.0.0.1:0"
        );
        let s = server.unwrap();
        assert_eq!(s.connected_count(), 0);
    }

    #[test]
    fn accept_connection_assigns_sequential_slots() {
        let mut server = RelayServer::bind("127.0.0.1:0", test_config()).unwrap();

        let addr1: SocketAddr = "127.0.0.1:10001".parse().unwrap();
        let addr2: SocketAddr = "127.0.0.1:10002".parse().unwrap();
        let addr3: SocketAddr = "127.0.0.1:10003".parse().unwrap();

        assert_eq!(server.accept_connection(addr1), Some(0));
        assert_eq!(server.accept_connection(addr2), Some(1));
        assert_eq!(server.accept_connection(addr3), Some(2));
        assert_eq!(server.connected_count(), 3);
    }

    #[test]
    fn accept_connection_returns_none_when_full() {
        let config = NetConfig {
            max_players: 4,
            ..test_config()
        };
        let mut server = RelayServer::bind("127.0.0.1:0", config).unwrap();

        for i in 0..4u16 {
            let addr: SocketAddr = format!("127.0.0.1:{}", 10001 + i).parse().unwrap();
            assert!(
                server.accept_connection(addr).is_some(),
                "slot {i} must be assignable"
            );
        }

        let overflow: SocketAddr = "127.0.0.1:10005".parse().unwrap();
        assert_eq!(
            server.accept_connection(overflow),
            None,
            "5th connection must be rejected when max_players=4"
        );
    }

    #[test]
    fn disconnect_player_clears_slot() {
        let mut server = RelayServer::bind("127.0.0.1:0", test_config()).unwrap();
        let addr: SocketAddr = "127.0.0.1:10001".parse().unwrap();

        let slot = server.accept_connection(addr).unwrap();
        assert_eq!(server.connected_count(), 1);

        server.disconnect_player(slot);
        assert_eq!(
            server.connected_count(),
            0,
            "disconnect must clear the slot"
        );

        // Slot should be reusable.
        let new_addr: SocketAddr = "127.0.0.1:10002".parse().unwrap();
        assert_eq!(
            server.accept_connection(new_addr),
            Some(0),
            "disconnected slot must be reusable"
        );
    }

    #[test]
    fn connected_count_reflects_active_connections() {
        let mut server = RelayServer::bind("127.0.0.1:0", test_config()).unwrap();
        assert_eq!(server.connected_count(), 0);

        let addr1: SocketAddr = "127.0.0.1:10001".parse().unwrap();
        let addr2: SocketAddr = "127.0.0.1:10002".parse().unwrap();

        server.accept_connection(addr1);
        assert_eq!(server.connected_count(), 1);

        server.accept_connection(addr2);
        assert_eq!(server.connected_count(), 2);

        server.disconnect_player(0);
        assert_eq!(server.connected_count(), 1);

        server.disconnect_player(1);
        assert_eq!(server.connected_count(), 0);
    }

    #[test]
    fn broadcast_excludes_sender() {
        use doom_types::TicCmd;

        // Create a server and two "client" transports.
        let mut server = RelayServer::bind("127.0.0.1:0", test_config()).unwrap();

        let mut client1 = NetTransport::bind("127.0.0.1:0").unwrap();
        let mut client2 = NetTransport::bind("127.0.0.1:0").unwrap();

        let addr1 = client1.local_addr().unwrap();
        let addr2 = client2.local_addr().unwrap();

        let slot1 = server.accept_connection(addr1).unwrap();
        let _slot2 = server.accept_connection(addr2).unwrap();

        let pkt = TicPacket {
            tic: 10,
            sender: slot1,
            ack_tic: 9,
            state_checksum: 0,
            cmds: [TicCmd::default(); MAX_PLAYERS],
        };

        // Broadcast excluding sender (slot 1 / client 1).
        server.broadcast_packet(&pkt, Some(slot1)).unwrap();

        // Client 2 should receive the packet.
        let recv2 = client2.recv_packet().unwrap();
        assert!(recv2.is_some(), "client2 must receive the broadcast");
        let (received, _) = recv2.unwrap();
        assert_eq!(received.tic, 10);

        // Client 1 should NOT receive the packet.
        let recv1 = client1.recv_packet().unwrap();
        assert!(recv1.is_none(), "sender must be excluded from broadcast");
    }

    #[test]
    fn check_timeouts_disconnects_stale_slots() {
        let config = NetConfig {
            timeout_ms: 1, // 1ms timeout for fast testing
            ..test_config()
        };
        let mut server = RelayServer::bind("127.0.0.1:0", config).unwrap();

        let addr: SocketAddr = "127.0.0.1:10001".parse().unwrap();
        server.accept_connection(addr);
        assert_eq!(server.connected_count(), 1);

        // Sleep just enough to exceed the 1ms timeout.
        std::thread::sleep(std::time::Duration::from_millis(10));

        server.check_timeouts();
        assert_eq!(
            server.connected_count(),
            0,
            "stale slot must be disconnected after timeout"
        );
    }
}
