//! Thin async UDP wrapper that speaks [`TicPacket`].
//!
//! [`UdpTransport`] owns a single bound `tokio::net::UdpSocket` and exposes
//! typed send/receive operations.  All I/O errors are mapped to [`NetError::Network`];
//! decode errors propagate as [`NetError::Decode`].

use std::net::SocketAddr;

use tokio::net::UdpSocket;

use crate::{NetError, packet::TicPacket};

/// Maximum wire size of a single UDP datagram we will send or accept.
///
/// A [`TicPacket`] with 4 players encoded as bincode 2 standard is well under
/// 512 bytes.  We reject (via decode error) anything that doesn't decode into a
/// valid [`TicPacket`] regardless.
pub const MAX_PACKET_BYTES: usize = 512;

// ---------------------------------------------------------------------------
// UdpTransport
// ---------------------------------------------------------------------------

/// A UDP socket that sends and receives [`TicPacket`] datagrams.
pub struct UdpTransport {
    socket: UdpSocket,
}

impl UdpTransport {
    /// Bind to `addr` (use port `0` to let the OS assign a free port).
    ///
    /// # Errors
    /// Returns [`NetError::Network`] if the bind call fails.
    pub async fn bind(addr: SocketAddr) -> Result<Self, NetError> {
        let socket = UdpSocket::bind(addr)
            .await
            .map_err(|e| NetError::Network(e.to_string()))?;
        Ok(Self { socket })
    }

    /// Encode `packet` and send it to `dest`.
    ///
    /// # Errors
    /// Returns [`NetError::Network`] on any I/O failure.
    pub async fn send_packet(&self, packet: &TicPacket, dest: SocketAddr) -> Result<(), NetError> {
        let bytes = packet.encode();
        self.socket
            .send_to(&bytes, dest)
            .await
            .map_err(|e| NetError::Network(e.to_string()))?;
        Ok(())
    }

    /// Wait for one datagram, decode it as a [`TicPacket`], and return it with
    /// the sender's address.
    ///
    /// # Errors
    /// - [`NetError::Network`] on socket I/O failure.
    /// - [`NetError::Decode`] if the datagram cannot be decoded as a [`TicPacket`].
    pub async fn recv_packet(&self) -> Result<(TicPacket, SocketAddr), NetError> {
        let mut buf = [0u8; MAX_PACKET_BYTES];
        let (len, addr) = self
            .socket
            .recv_from(&mut buf)
            .await
            .map_err(|e| NetError::Network(e.to_string()))?;
        let packet = TicPacket::decode(&buf[..len])?;
        Ok((packet, addr))
    }

    /// Returns the local address this socket is bound to.
    ///
    /// # Errors
    /// Returns [`NetError::Network`] if the OS query fails.
    pub fn local_addr(&self) -> Result<SocketAddr, NetError> {
        self.socket
            .local_addr()
            .map_err(|e| NetError::Network(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{TicPacket, WireTicCmd, MAX_PLAYERS};

    fn sample_packet(tic: u32, sender: u8) -> TicPacket {
        let mut cmds = [WireTicCmd::default(); MAX_PLAYERS];
        cmds[sender as usize % MAX_PLAYERS] = WireTicCmd {
            forward_move: 42,
            side_move: -7,
            angle_turn: 256,
            buttons: 0x01,
            chatchar: 0,
        };
        TicPacket {
            tic,
            cmds,
            state_checksum: 0xCAFE_BABE,
            sender,
            ack_tic: tic.saturating_sub(1),
        }
    }

    /// Binding to `127.0.0.1:0` must succeed and `local_addr()` must return Ok.
    #[tokio::test]
    async fn transport_bind_localhost() {
        let addr: SocketAddr = "127.0.0.1:0".parse().expect("valid addr");
        let transport = UdpTransport::bind(addr).await.expect("bind must succeed");
        let local = transport.local_addr().expect("local_addr must return Ok");
        // OS assigns a non-zero port.
        assert_ne!(local.port(), 0, "OS must assign a non-zero port");
    }

    /// Send a TicPacket from one socket and receive it on another; the decoded
    /// packet must equal what was sent.
    #[tokio::test]
    async fn transport_send_recv_loopback() {
        let any: SocketAddr = "127.0.0.1:0".parse().expect("valid addr");

        let sender = UdpTransport::bind(any).await.expect("sender bind");
        let receiver = UdpTransport::bind(any).await.expect("receiver bind");

        let recv_addr = receiver.local_addr().expect("receiver local addr");
        let packet = sample_packet(7, 0);

        sender
            .send_packet(&packet, recv_addr)
            .await
            .expect("send must succeed");

        let (got, from) = receiver
            .recv_packet()
            .await
            .expect("recv must succeed");

        assert_eq!(got, packet, "received packet must equal sent packet");
        assert_eq!(
            from.ip(),
            sender.local_addr().expect("sender local addr").ip(),
            "sender IP must match"
        );
    }

    /// Sending raw garbage bytes must cause `recv_packet` to return `Err`, not panic.
    #[tokio::test]
    async fn transport_decode_garbage_returns_err() {
        let any: SocketAddr = "127.0.0.1:0".parse().expect("valid addr");

        let sender_sock = tokio::net::UdpSocket::bind(any).await.expect("bind");
        let receiver = UdpTransport::bind(any).await.expect("receiver bind");
        let recv_addr = receiver.local_addr().expect("receiver local addr");

        // Send bytes that cannot decode as a TicPacket.
        let garbage = [0xFFu8; 3];
        sender_sock
            .send_to(&garbage, recv_addr)
            .await
            .expect("raw send must succeed");

        let result = receiver.recv_packet().await;
        assert!(
            result.is_err(),
            "recv_packet on garbage bytes must return Err, not panic"
        );
    }
}
