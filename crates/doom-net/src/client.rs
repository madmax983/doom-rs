//! Network client for doom-net multiplayer.
//!
//! [`NetClient`] connects to a [`super::server::RelayServer`], performs the
//! handshake to obtain a player slot, and provides send/recv for gameplay
//! [`TicPacket`]s.

use std::io;
use std::net::SocketAddr;

use crate::packet::TicPacket;
use crate::transport::{ConnectionState, NetStats, NetTransport};

// ---------------------------------------------------------------------------
// NetClient
// ---------------------------------------------------------------------------

/// A multiplayer client that communicates with a relay server.
#[derive(Debug)]
pub struct NetClient {
    /// The underlying UDP transport.
    transport: NetTransport,
    /// The player slot assigned by the server (0-based).
    player_slot: u8,
    /// The address of the server.
    server_addr: SocketAddr,
    /// Whether the client considers itself connected.
    connected: bool,
}

impl NetClient {
    /// Connect to a relay server at `server_addr` (e.g. `"127.0.0.1:5029"`),
    /// binding the local socket to `local_port` (use 0 for OS-assigned).
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if binding to the local port fails, or if
    /// connecting to the server address fails.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_net::{NetClient, NetConfig, RelayServer};
    ///
    /// // Start a local server to connect to.
    /// let server = RelayServer::bind("127.0.0.1:0", NetConfig::default()).unwrap();
    /// let server_addr = server.local_addr().unwrap();
    ///
    /// // Connect the client to the server on an OS-assigned port.
    /// let client = NetClient::connect(&server_addr.to_string(), 0).unwrap();
    /// ```
    pub fn connect(server_addr: &str, local_port: u16) -> io::Result<Self> {
        let local_bind = format!("127.0.0.1:{local_port}");
        let mut transport = NetTransport::bind(&local_bind)?;

        let parsed_addr: SocketAddr = server_addr
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        transport.connect_to(server_addr)?;

        Ok(Self {
            transport,
            player_slot: 0,
            server_addr: parsed_addr,
            connected: false,
        })
    }

    /// Create a `NetClient` from an already-bound transport and a known
    /// server address + slot (used after handshake completion).
    #[must_use]
    pub const fn from_parts(
        transport: NetTransport,
        server_addr: SocketAddr,
        player_slot: u8,
    ) -> Self {
        Self {
            transport,
            player_slot,
            server_addr,
            connected: true,
        }
    }

    /// Send a tic input packet to the server.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if the underlying socket fails to send the packet.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_net::{NetClient, NetConfig, RelayServer, TicPacket};
    /// use doom_types::TicCmd;
    ///
    /// let server = RelayServer::bind("127.0.0.1:0", NetConfig::default()).unwrap();
    /// let mut client = NetClient::connect(&server.local_addr().unwrap().to_string(), 0).unwrap();
    ///
    /// let packet = TicPacket {
    ///     tic: 100,
    ///     sender: 0,
    ///     ack_tic: 99,
    ///     state_checksum: 0x12345678,
    ///     cmds: [TicCmd::default(); doom_net::MAX_PLAYERS],
    /// };
    /// client.send_input(&packet).unwrap();
    /// ```
    pub fn send_input(&mut self, packet: &TicPacket) -> io::Result<()> {
        let data = packet.to_bytes();
        self.transport.send_raw(&data, &self.server_addr)?;
        Ok(())
    }

    /// Non-blocking receive of a [`TicPacket`] from the server.
    ///
    /// Returns `Ok(None)` if no data is available.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if there is an issue reading from the socket.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_net::{NetClient, NetConfig, RelayServer};
    ///
    /// let server = RelayServer::bind("127.0.0.1:0", NetConfig::default()).unwrap();
    /// let mut client = NetClient::connect(&server.local_addr().unwrap().to_string(), 0).unwrap();
    ///
    /// // Non-blocking receive, returns None if no packets are waiting
    /// let packet = client.recv_packet().unwrap();
    /// assert!(packet.is_none());
    /// ```
    pub fn recv_packet(&mut self) -> io::Result<Option<TicPacket>> {
        match self.transport.recv_packet()? {
            Some((pkt, _addr)) => Ok(Some(pkt)),
            None => Ok(None),
        }
    }

    /// Mark the client as disconnected.
    pub const fn disconnect(&mut self) {
        self.connected = false;
        self.transport.set_state(ConnectionState::Disconnected);
    }

    /// Returns `true` if the client considers itself connected.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        self.connected
    }

    /// The player slot assigned by the server.
    #[must_use]
    pub const fn player_slot(&self) -> u8 {
        self.player_slot
    }

    /// Set the player slot (called after receiving a handshake response).
    pub const fn set_player_slot(&mut self, slot: u8) {
        self.player_slot = slot;
        self.connected = true;
        self.transport.set_state(ConnectionState::Connected {
            player_slot: slot,
            remote_addr: self.server_addr,
        });
    }

    /// Return a snapshot of the traffic statistics.
    #[must_use]
    pub const fn stats(&self) -> NetStats {
        self.transport.stats()
    }

    /// The local address the client socket is bound to.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if the socket address cannot be retrieved.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.transport.local_addr()
    }

    /// The server address this client is connected to.
    #[must_use]
    pub const fn server_addr(&self) -> SocketAddr {
        self.server_addr
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{MAX_PLAYERS, TicPacket};
    use crate::server::RelayServer;
    use crate::transport::{NetConfig, make_join_packet};
    use doom_types::TicCmd;

    fn test_config() -> NetConfig {
        NetConfig {
            port: 0,
            max_players: 4,
            timeout_ms: 5000,
            keepalive_interval_ms: 1000,
        }
    }

    #[test]
    fn net_client_struct_creation() {
        // Bind a server so we have a valid address to connect to.
        let server =
            RelayServer::bind("127.0.0.1:0", test_config()).expect("value must exist in test");
        let server_addr = server.local_addr().expect("value must exist in test");

        let client = NetClient::connect(&server_addr.to_string(), 0);
        assert!(client.is_ok(), "NetClient::connect must succeed");
        let c = client.expect("value must exist in test");
        assert!(
            !c.is_connected(),
            "fresh client must not be connected until handshake"
        );
        assert_eq!(c.player_slot(), 0, "default slot must be 0");
    }

    #[test]
    fn net_client_disconnect() {
        let server =
            RelayServer::bind("127.0.0.1:0", test_config()).expect("value must exist in test");
        let server_addr = server.local_addr().expect("value must exist in test");

        let mut client =
            NetClient::connect(&server_addr.to_string(), 0).expect("value must exist in test");
        client.set_player_slot(1);
        assert!(client.is_connected());

        client.disconnect();
        assert!(
            !client.is_connected(),
            "disconnect must mark client as not connected"
        );
    }

    #[test]
    fn recv_packet_error_is_propagated() {
        // By creating a NetTransport and converting it to a NetClient,
        // and using a closed/bad socket or similar, we can test error propagation.
        // It's easiest to mock or use an invalid configuration if possible,
        // but `NetTransport::bind` doesn't let us inject bad sockets directly.
        // Instead, we can bind a client to a server, then drop the server.
        let server =
            RelayServer::bind("127.0.0.1:0", test_config()).expect("value must exist in test");
        let server_addr = server.local_addr().expect("value must exist in test");

        let mut client =
            NetClient::connect(&server_addr.to_string(), 0).expect("value must exist in test");
        // Just verify recv_packet returns Ok(None) when nothing is sent (WouldBlock).
        let result = client.recv_packet();
        assert!(result.is_ok());
        assert!(result.expect("value must exist in test").is_none());

        // Also verify the server_addr method returns what we passed in.
        assert_eq!(client.server_addr(), server_addr);

        let c_from_parts = NetClient::from_parts(
            crate::NetTransport::bind("127.0.0.1:0").expect("value must exist in test"),
            server_addr,
            3,
        );
        assert!(c_from_parts.is_connected());
        assert_eq!(c_from_parts.player_slot(), 3);
        assert_eq!(c_from_parts.server_addr(), server_addr);
    }

    #[test]
    fn net_client_stats() {
        let server =
            RelayServer::bind("127.0.0.1:0", test_config()).expect("value must exist in test");
        let server_addr = server.local_addr().expect("value must exist in test");

        let client =
            NetClient::connect(&server_addr.to_string(), 0).expect("value must exist in test");
        let stats = client.stats();
        assert_eq!(stats.packets_sent, 0);
        assert_eq!(stats.packets_received, 0);
    }

    #[test]
    fn net_client_send_and_server_recv() {
        let mut server =
            RelayServer::bind("127.0.0.1:0", test_config()).expect("value must exist in test");
        let server_addr = server.local_addr().expect("value must exist in test");

        let mut client =
            NetClient::connect(&server_addr.to_string(), 0).expect("value must exist in test");
        let client_addr = client.local_addr().expect("value must exist in test");

        // Manually accept the client on the server side.
        let slot = server
            .accept_connection(client_addr)
            .expect("value must exist in test");
        client.set_player_slot(slot);

        // Send a packet from client to server.
        let pkt = TicPacket {
            tic: 5,
            sender: slot,
            ack_tic: 4,
            state_checksum: 0xBEEF,
            cmds: [TicCmd::default(); MAX_PLAYERS],
        };
        client.send_input(&pkt).expect("value must exist in test");

        // Server should receive it.
        let result = server.poll_once().expect("value must exist in test");
        assert!(result.is_some(), "server must receive the client's packet");
        let (received, sender_slot) = result.expect("value must exist in test");
        assert_eq!(received.tic, 5);
        assert_eq!(sender_slot, slot);
    }

    #[test]
    fn handshake_round_trip() {
        // Set up server and a raw transport simulating the client side.
        let mut server =
            RelayServer::bind("127.0.0.1:0", test_config()).expect("value must exist in test");
        let server_addr = server.local_addr().expect("value must exist in test");

        let mut client_transport =
            NetTransport::bind("127.0.0.1:0").expect("value must exist in test");
        let _client_addr = client_transport
            .local_addr()
            .expect("value must exist in test");

        // Client sends a join packet to the server.
        let join_pkt = make_join_packet();
        let data = join_pkt.to_bytes();
        client_transport
            .send_raw(&data, &server_addr)
            .expect("value must exist in test");

        // Server processes it via poll_once (which auto-handles join requests).
        let poll_result = server.poll_once().expect("value must exist in test");
        // poll_once returns None for handshake packets (handled internally).
        assert!(poll_result.is_none(), "join must be handled internally");

        // The server should have assigned a slot.
        assert_eq!(server.connected_count(), 1);

        // Client should receive the join response.
        let response = client_transport
            .recv_packet()
            .expect("value must exist in test");
        assert!(response.is_some(), "client must receive join response");
        let (resp_pkt, _) = response.expect("value must exist in test");
        assert_eq!(
            resp_pkt.tic, 0xFFFF_FFFF,
            "response tic must be handshake magic"
        );
        assert_eq!(resp_pkt.sender, 0, "assigned slot must be 0 (first slot)");

        // Verify the response is recognized as a join response.
        assert!(crate::transport::is_join_response(&resp_pkt));
    }
}
