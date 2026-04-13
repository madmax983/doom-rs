//! Network play modes: relay server and networked game client wrapper.
//!
//! This module provides:
//! - Conversion helpers between doom-tui's `TicInput`, doom-game's `TicCmd`,
//!   and the wire-format `doom_net::TicCmd`.
//! - [`run_server`] — standalone relay server loop using [`doom_net::RelayServer`].
//! - [`NetGameApp`] — `DoomApp` wrapper that integrates a [`doom_net::NetClient`]
//!   into the game loop for client-side netplay.

use anyhow::Result;
use doom_net::{MAX_PLAYERS, NetClient, NetConfig, RelayServer, TicPacket};
use doom_renderer::Framebuffer;
use doom_tui::{DoomApp, TicInput};
use std::time::{Duration, Instant};

use crate::DoomGame;

/// Default UDP port for the doom-rs relay server.
#[cfg(test)]
pub(crate) const DEFAULT_PORT: u16 = 5029;

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
    use crossterm::{
        event::{self, Event, KeyCode, KeyModifiers},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    };
    use ratatui::{
        Terminal,
        backend::CrosstermBackend,
        layout::{Constraint, Direction, Layout},
        style::{Color, Modifier, Style},
        text::Span,
        widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    };
    use std::io::stdout;

    let config = NetConfig {
        port,
        ..NetConfig::default()
    };
    let mut server = RelayServer::new(config)
        .map_err(|e| anyhow::anyhow!("Failed to bind relay server on port {port}: {e}"))?;

    enable_raw_mode()?;
    let mut stdout_handle = stdout();
    execute!(stdout_handle, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout_handle);
    let mut terminal = Terminal::new(backend)?;

    let mut logs: Vec<String> = vec![
        format!("doom-rs relay server listening on port {port}"),
        "Press 'q' or Ctrl+C to stop.".to_string(),
    ];
    let mut prev_count = 0usize;
    let mut last_draw = Instant::now();
    let mut needs_draw = true;

    let res = (|| -> Result<()> {
        loop {
            // Check for exit
            if event::poll(Duration::from_millis(1))? {
                if let Event::Key(key) = event::read()? {
                    if key.code == KeyCode::Char('q')
                        || (key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL))
                    {
                        break;
                    }
                }
            }

            // Poll for one packet (handles join handshakes internally).
            match server.poll_once() {
                Ok(Some((packet, sender_slot))) => {
                    // Relay the packet to all other connected clients.
                    let _ = server.broadcast_packet(&packet, Some(sender_slot));
                }
                Ok(None) => {
                    // No data this iteration — yield to avoid busy-spinning.
                    // (The terminal poll already yielded, so no need for explicit sleep here)
                }
                Err(e) => {
                    logs.push(format!("Server poll error: {e}"));
                    if logs.len() > 100 {
                        logs.remove(0);
                    }
                    needs_draw = true;
                }
            }

            // Periodically check for stale connections.
            server.check_timeouts();

            // Log connection count changes.
            let count = server.connected_count();
            if count != prev_count {
                logs.push(format!("Connected players: {count}"));
                if logs.len() > 100 {
                    logs.remove(0);
                }
                prev_count = count;
                needs_draw = true;
            }

            // Limit draw rate to ~30 FPS (33ms) or state changes to avoid CPU spiking.
            if needs_draw || last_draw.elapsed() >= Duration::from_millis(33) {
                terminal.draw(|f| {
                    let size = f.area();
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .margin(1)
                        .constraints([
                            Constraint::Length(3),
                            Constraint::Length(3),
                            Constraint::Min(5),
                        ])
                        .split(size);

                    let header = Paragraph::new(Span::styled(
                        format!("doom-rs Relay Server - Port {}", port),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ))
                    .block(Block::default().borders(Borders::ALL));
                    f.render_widget(header, chunks[0]);

                    let ratio = count as f64 / MAX_PLAYERS as f64;
                    let gauge = Gauge::default()
                        .block(
                            Block::default()
                                .title("Connected Players")
                                .borders(Borders::ALL),
                        )
                        .gauge_style(Style::default().fg(Color::Yellow))
                        .ratio(ratio.clamp(0.0, 1.0))
                        .label(format!("{} / {}", count, MAX_PLAYERS));
                    f.render_widget(gauge, chunks[1]);

                    let items: Vec<ListItem> = logs
                        .iter()
                        .rev()
                        .map(|msg| ListItem::new(Span::raw(msg)))
                        .collect();
                    let list = List::new(items).block(
                        Block::default()
                            .title("Recent Events")
                            .borders(Borders::ALL),
                    );
                    f.render_widget(list, chunks[2]);
                })?;
                needs_draw = false;
                last_draw = Instant::now();
            }
        }
        Ok(())
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
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
        let wire_cmd = input.into();
        let slot = self.client.player_slot();
        let mut cmds = [doom_types::TicCmd::default(); MAX_PLAYERS];
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
                server_pkt.cmds[slot as usize]
            } else {
                input.into()
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
        let s = server.expect("server bind must succeed");
        assert_eq!(
            s.connected_count(),
            0,
            "fresh server must have 0 connections"
        );
    }

    #[test]
    fn default_port_is_5029() {
        assert_eq!(DEFAULT_PORT, 5029, "default netplay port must be 5029");
        let cfg = NetConfig::default();
        assert_eq!(
            cfg.port, DEFAULT_PORT,
            "NetConfig default port must match DEFAULT_PORT"
        );
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
        let server = RelayServer::bind("127.0.0.1:0", config).expect("server bind must succeed");
        let server_addr = server.local_addr().expect("server must have local addr");
        let client =
            NetClient::connect(&server_addr.to_string(), 0).expect("client connect must succeed");

        let net_app = NetGameApp::new(game, client);

        // Verify the wrapper exposes the inner game.
        assert!(
            !net_app.inner().automap.active,
            "automap must start inactive through wrapper"
        );
        assert_eq!(net_app.tic(), 0, "tic counter must start at 0");
        assert!(
            !net_app.client().is_connected(),
            "client starts unhandshaked"
        );
    }

    #[test]
    fn net_game_app_tick_advances_tic_counter() {
        let game = crate::tests::make_doom_game();
        let config = NetConfig {
            port: 0,
            ..NetConfig::default()
        };
        let server = RelayServer::bind("127.0.0.1:0", config).expect("server bind must succeed");
        let server_addr = server.local_addr().expect("server must have local addr");
        let client =
            NetClient::connect(&server_addr.to_string(), 0).expect("client connect must succeed");

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
        let server = RelayServer::bind("127.0.0.1:0", config).expect("server bind must succeed");
        let server_addr = server.local_addr().expect("server must have local addr");
        let client =
            NetClient::connect(&server_addr.to_string(), 0).expect("client connect must succeed");

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
        let server = RelayServer::bind("127.0.0.1:0", config).expect("server bind must succeed");
        let server_addr = server.local_addr().expect("server must have local addr");
        let client =
            NetClient::connect(&server_addr.to_string(), 0).expect("client connect must succeed");

        let mut net_app = NetGameApp::new(game, client);
        let mut fb = Framebuffer::new();
        // Should not panic even with no server response.
        net_app.render(&mut fb);
    }
}
