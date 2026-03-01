//! Network play modes: relay server and networked game client wrapper.
//!
//! # Server mode
//! Run with `--server <port>` to start a headless relay server that collects
//! per-tic inputs from all players and broadcasts the authoritative packet.
//!
//! # Client mode
//! Run with `--connect <addr>` to join a relay server.  The [`NetGameApp`]
//! wrapper implements [`DoomApp`] and drives the game simulation with
//! authoritative server inputs (with prediction when the server hasn't
//! responded yet).

use std::net::SocketAddr;

use doom_game::TicCmd;
use doom_net::WireTicCmd;
use doom_renderer::Framebuffer;
use doom_tui::{DoomApp, TicInput};

use crate::DoomGame;

// ---------------------------------------------------------------------------
// Server mode
// ---------------------------------------------------------------------------

/// Run the relay server standalone (no rendering, no game state).
///
/// Blocks until the process is interrupted; each call to
/// [`doom_net::RelayServer::step`] processes one incoming UDP packet and
/// broadcasts an authoritative packet when a tic is complete.
///
/// # Errors
/// Returns an error if the socket bind fails or a fatal receive error occurs.
pub async fn run_server(port: u16, num_players: u8) -> anyhow::Result<()> {
    use doom_net::RelayServer;

    let addr: SocketAddr = format!("0.0.0.0:{port}")
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid server address: {e}"))?;

    println!("Doom relay server listening on :{port} (waiting for {num_players} player(s))");

    let mut server = RelayServer::bind(addr, num_players).await
        .map_err(|e| anyhow::anyhow!("Server bind failed: {e}"))?;

    loop {
        match server.step().await {
            Ok(Some(tic)) => {
                // Authoritative tic broadcast — log occasionally to show progress.
                if tic % 350 == 0 {
                    println!("Server: tic {tic} complete");
                }
            }
            Ok(None) => {
                // Tic not yet complete (still waiting for other players).
            }
            Err(e) => {
                eprintln!("Server step error: {e}");
                return Err(anyhow::anyhow!("Server error: {e}"));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// NetGameApp — DoomApp wrapper for network play
// ---------------------------------------------------------------------------

/// Wraps [`DoomGame`] for network play.
///
/// Each tic:
/// 1. Convert live [`TicInput`] to a [`TicCmd`] and send it to the server.
/// 2. Poll the server for an authoritative packet (1 ms timeout).
/// 3. If authoritative input received: use it; otherwise predict (repeat last
///    received command — "ghost rollback light").
/// 4. Tick the game with the (possibly predicted) command.
pub struct NetGameApp {
    inner: DoomGame,
    client: doom_net::NetClient,
    player_num: u8,
    last_wire_cmd: WireTicCmd,
    rt: tokio::runtime::Runtime,
}

impl NetGameApp {
    /// Create a new networked game wrapper.
    ///
    /// `player_num` is 1-based (matches the CLI `--player` argument).
    pub fn new(inner: DoomGame, client: doom_net::NetClient, player_num: u8) -> Self {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        Self {
            inner,
            client,
            player_num,
            last_wire_cmd: WireTicCmd::default(),
            rt,
        }
    }
}

impl DoomApp for NetGameApp {
    fn tick(&mut self, input: TicInput) {
        // Convert live input to TicCmd and send to server.
        let local_cmd = ticinput_to_ticcmd(input);

        self.rt.block_on(async {
            // send_input records the cmd in the log and advances current_tic.
            let _ = self.client.send_input(local_cmd).await;
        });

        // Poll for authoritative input (1 ms timeout built into poll_authoritative).
        let auth = self.rt.block_on(async {
            self.client.poll_authoritative().await
        });

        // Use authoritative if available; otherwise predict with last received cmd.
        let wire_cmd = match auth {
            Some((_tic, cmds)) => {
                // player_num is 1-based; player_index in the array is 0-based.
                let idx = (self.player_num.saturating_sub(1)) as usize;
                let cmd = cmds.get(idx).copied().unwrap_or_default();
                self.last_wire_cmd = cmd;
                cmd
            }
            None => self.last_wire_cmd,
        };

        // Convert WireTicCmd → TicCmd and tick the simulation.
        let cmd: TicCmd = wire_cmd.into();
        self.inner.gs.tick(cmd, Some(&mut self.inner.level));
    }

    fn render(&mut self, fb: &mut Framebuffer) {
        self.inner.render(fb);
    }

    fn active_palette(&self) -> usize {
        self.inner.active_palette()
    }
}

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

/// Convert a [`TicInput`] to the wire format [`WireTicCmd`].
#[allow(dead_code)]
pub(crate) fn ticinput_to_wire(input: TicInput) -> WireTicCmd {
    WireTicCmd {
        forward_move: input.forward_move,
        side_move: input.side_move,
        angle_turn: input.angle_turn,
        buttons: input.buttons,
        chatchar: input.chatchar,
    }
}

/// Convert a [`WireTicCmd`] to a [`TicCmd`].
///
/// Uses the `From` impl in doom-net's `packet.rs`.
#[allow(dead_code)]
pub(crate) fn wire_to_ticcmd(w: WireTicCmd) -> TicCmd {
    w.into()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
        let wire = WireTicCmd {
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
        let wire = WireTicCmd::default();
        assert_eq!(wire.forward_move, 0);
        assert_eq!(wire.side_move, 0);
        assert_eq!(wire.angle_turn, 0);
        assert_eq!(wire.buttons, 0);
        assert_eq!(wire.chatchar, 0);
    }

    #[test]
    fn wire_cmd_into_ticcmd_preserves_chatchar() {
        let wire = WireTicCmd {
            chatchar: b'X',
            ..Default::default()
        };
        let cmd: TicCmd = wire.into();
        assert_eq!(cmd.chatchar, b'X');
    }
}
