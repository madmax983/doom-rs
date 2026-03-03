//! Network play modes: relay server and networked game client wrapper.
//!
//! This module provides:
//! - Conversion helpers between doom-tui's `TicInput`, doom-game's `TicCmd`,
//!   and the wire-format `doom_net::TicCmd`.
//! - [`run_server`] — standalone relay server loop using [`doom_net::RelayServer`].
//! - [`NetGameApp`] — `DoomApp` wrapper that integrates a [`doom_net::NetClient`]
//!   into the game loop for client-side netplay.

use anyhow::Result;
use doom_game::TicCmd;
use doom_net::{NetClient, NetConfig, RelayServer, TicPacket, MAX_PLAYERS};
use doom_renderer::Framebuffer;
use doom_tui::{DoomApp, TicInput};

use crate::DoomGame;

/// Default UDP port for the doom-rs relay server.
pub(crate) const DEFAULT_PORT: u16 = 5029;

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
// Relay server
// ---------------------------------------------------------------------------

/// Run a standalone relay server on the specified `port`.
///
/// The server accepts client connections, relays [`TicPacket`]s between all
/// connected peers, and periodically disconnects stale clients.  It runs
/// until an I/O error occurs (or the process is terminated).
///
/// No WAD file or game simulation is needed server-side.
pub(crate) fn run_server(port: u16) -> Result<()> {
    let config = NetConfig {
        port,
        ..NetConfig::default()
    };
    let mut server = RelayServer::new(config)
        .map_err(|e| anyhow::anyhow!("Failed to bind relay server on port {port}: {e}"))?;

    eprintln!("doom-rs relay server listening on port {port}");
    eprintln!("Press Ctrl+C to stop.");

    let mut prev_count = 0usize;
    loop {
        // Poll for one packet (handles join handshakes internally).
        match server.poll_once() {
            Ok(Some((packet, sender_slot))) => {
                // Relay the packet to all other connected clients.
                let _ = server.broadcast_packet(&packet, Some(sender_slot));
            }
            Ok(None) => {
                // No data this iteration — yield to avoid busy-spinning.
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(e) => {
                eprintln!("Server poll error: {e}");
            }
        }

        // Periodically check for stale connections.
        server.check_timeouts();

        // Log connection count changes.
        let count = server.connected_count();
        if count != prev_count {
            eprintln!("Connected players: {count}");
            prev_count = count;
        }
    }
}

// ---------------------------------------------------------------------------
// NetGameApp — network-aware DoomApp wrapper
// ---------------------------------------------------------------------------

/// Network-aware game wrapper that sends local input to the server and
/// applies authoritative inputs received from the relay.
///
/// Implements [`DoomApp`] so it can be plugged directly into
/// [`doom_tui::DoomEventLoop`].
pub(crate) struct NetGameApp {
    /// The inner single-player game.
    inner: DoomGame,
    /// Network client connected to the relay server.
    client: NetClient,
    /// Current tic number (monotonically increasing).
    tic: u32,
}

impl NetGameApp {
    /// Create a new network game wrapper around an existing `DoomGame` and
    /// connected `NetClient`.
    pub(crate) fn new(inner: DoomGame, client: NetClient) -> Self {
        Self {
            inner,
            client,
            tic: 0,
        }
    }

    /// Borrow the inner `DoomGame` (for tests).
    #[cfg(test)]
    pub(crate) fn inner(&self) -> &DoomGame {
        &self.inner
    }

    /// Borrow the `NetClient` (for tests).
    #[cfg(test)]
    pub(crate) fn client(&self) -> &NetClient {
        &self.client
    }

    /// The current tic number.
    #[cfg(test)]
    pub(crate) fn tic(&self) -> u32 {
        self.tic
    }
}

impl DoomApp for NetGameApp {
    fn tick(&mut self, input: TicInput) {
        // Convert local input to wire format and send to the server.
        let wire_cmd = ticinput_to_wire(input);
        let slot = self.client.player_slot();
        let mut cmds = [doom_net::TicCmd::default(); MAX_PLAYERS];
        if (slot as usize) < MAX_PLAYERS {
            cmds[slot as usize] = wire_cmd;
        }

        let packet = TicPacket {
            tic: self.tic,
            sender: slot,
            ack_tic: self.tic.saturating_sub(1),
            state_checksum: 0,
            cmds,
        };
        let _ = self.client.send_input(&packet);

        // Try to receive authoritative inputs from the server.
        // Apply them if available; otherwise fall through with local input.
        if let Ok(Some(server_pkt)) = self.client.recv_packet() {
            // Apply the authoritative command for our slot (or the first
            // non-zero command).  For now, use our own slot's command.
            let auth_cmd = if (slot as usize) < MAX_PLAYERS {
                wire_to_ticcmd(server_pkt.cmds[slot as usize])
            } else {
                ticinput_to_ticcmd(input)
            };

            // Feed the authoritative command into the game via a synthetic
            // TicInput so that cheats, console, etc. still work from the
            // original input.
            let mut auth_input = input;
            auth_input.forward_move = auth_cmd.forward_move;
            auth_input.side_move = auth_cmd.side_move;
            auth_input.angle_turn = auth_cmd.angle_turn;
            auth_input.buttons = auth_cmd.buttons;
            self.inner.tick(auth_input);
        } else {
            // No server packet yet — tick with local prediction.
            self.inner.tick(input);
        }

        self.tic += 1;
    }

    fn render(&mut self, fb: &mut Framebuffer) {
        self.inner.render(fb);
    }

    fn active_palette(&self) -> usize {
        self.inner.active_palette()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Conversion helper tests (preserved from original) --

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

    // -- Server tests --

    #[test]
    fn relay_server_creates_with_custom_port() {
        // Use port 0 (OS-assigned) so the test doesn't clash with real services.
        let config = NetConfig {
            port: 0,
            ..NetConfig::default()
        };
        let server = RelayServer::bind("127.0.0.1:0", config);
        assert!(server.is_ok(), "RelayServer must bind successfully");
        let s = server.unwrap();
        assert_eq!(s.connected_count(), 0, "fresh server must have 0 connections");
    }

    #[test]
    fn default_port_is_5029() {
        assert_eq!(DEFAULT_PORT, 5029, "default netplay port must be 5029");
        let cfg = NetConfig::default();
        assert_eq!(cfg.port, DEFAULT_PORT, "NetConfig default port must match DEFAULT_PORT");
    }

    // -- NetGameApp tests --

    #[test]
    fn net_game_app_wraps_doom_game() {
        let game = crate::tests::make_doom_game();

        // Create a dummy server + client pair for testing.
        let config = NetConfig {
            port: 0,
            ..NetConfig::default()
        };
        let server = RelayServer::bind("127.0.0.1:0", config).unwrap();
        let server_addr = server.local_addr().unwrap();
        let client = NetClient::connect(&server_addr.to_string(), 0).unwrap();

        let net_app = NetGameApp::new(game, client);

        // Verify the wrapper exposes the inner game.
        assert!(!net_app.inner().automap.active, "automap must start inactive through wrapper");
        assert_eq!(net_app.tic(), 0, "tic counter must start at 0");
        assert!(!net_app.client().is_connected(), "client starts unhandshaked");
    }

    #[test]
    fn net_game_app_tick_advances_tic_counter() {
        let game = crate::tests::make_doom_game();
        let config = NetConfig {
            port: 0,
            ..NetConfig::default()
        };
        let server = RelayServer::bind("127.0.0.1:0", config).unwrap();
        let server_addr = server.local_addr().unwrap();
        let client = NetClient::connect(&server_addr.to_string(), 0).unwrap();

        let mut net_app = NetGameApp::new(game, client);
        assert_eq!(net_app.tic(), 0);

        net_app.tick(TicInput::default());
        assert_eq!(net_app.tic(), 1, "tic must advance to 1 after one tick");

        net_app.tick(TicInput::default());
        assert_eq!(net_app.tic(), 2, "tic must advance to 2 after two ticks");
    }

    #[test]
    fn net_game_app_active_palette_delegates() {
        let game = crate::tests::make_doom_game();
        let config = NetConfig {
            port: 0,
            ..NetConfig::default()
        };
        let server = RelayServer::bind("127.0.0.1:0", config).unwrap();
        let server_addr = server.local_addr().unwrap();
        let client = NetClient::connect(&server_addr.to_string(), 0).unwrap();

        let net_app = NetGameApp::new(game, client);
        assert_eq!(
            net_app.active_palette(),
            0,
            "active_palette must delegate to inner DoomGame"
        );
    }

    #[test]
    fn net_game_app_render_does_not_panic() {
        let game = crate::tests::make_doom_game();
        let config = NetConfig {
            port: 0,
            ..NetConfig::default()
        };
        let server = RelayServer::bind("127.0.0.1:0", config).unwrap();
        let server_addr = server.local_addr().unwrap();
        let client = NetClient::connect(&server_addr.to_string(), 0).unwrap();

        let mut net_app = NetGameApp::new(game, client);
        let mut fb = Framebuffer::new();
        // Should not panic even with no server response.
        net_app.render(&mut fb);
    }
}
