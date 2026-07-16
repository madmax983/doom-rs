//! APM (Actions Per Minute) and Input Analytics for Doom Demos.
//!
//! This module analyzes a demo stream to calculate how fast and active
//! a player is, measuring total actions and Actions Per Minute (APM).

use crate::DemoPlayer;
use crate::ticcmd::DemoTicCmd;

/// Analytics data for a single player in a demo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerAnalytics {
    /// Total number of tics parsed.
    pub total_tics: u32,
    /// Total number of tics where the player was performing an action or moving.
    pub active_tics: u32,
    /// Total discrete input actions (button presses, movement direction changes).
    pub total_actions: u32,
    /// Calculated Actions Per Minute based on 35 tics per second.
    pub apm: f64,
}

/// Analyzes a demo and extracts input statistics.
pub struct ApmAnalyzer {
    /// Analytics for each player present in the demo.
    pub players: Vec<PlayerAnalytics>,
}

impl ApmAnalyzer {
    /// Analyzes a full demo, returning APM and action stats for all players.
    #[must_use]
    pub fn analyze(player: &mut DemoPlayer) -> Self {
        player.reset();

        let players = Self::analyze_cmds(std::iter::from_fn(|| player.next_tic_cmds()));

        player.reset();

        Self { players }
    }

    /// Core logic extracted for testing without needing full demo structures.
    pub fn analyze_cmds(cmds_iter: impl Iterator<Item = Vec<DemoTicCmd>>) -> Vec<PlayerAnalytics> {
        let mut players: Vec<PlayerAnalytics> = Vec::new();
        let mut last_cmds: Vec<DemoTicCmd> = Vec::new();

        for cmds in cmds_iter {
            for (i, cmd) in cmds.iter().enumerate() {
                if i >= players.len() {
                    players.push(PlayerAnalytics::default());
                    last_cmds.push(DemoTicCmd::default());
                }

                players[i].total_tics += 1;

                let mut is_active = false;

                if cmd.forward_move != last_cmds[i].forward_move {
                    players[i].total_actions += 1;
                    is_active = true;
                }

                if cmd.side_move != last_cmds[i].side_move {
                    players[i].total_actions += 1;
                    is_active = true;
                }

                if cmd.angle_turn != last_cmds[i].angle_turn {
                    players[i].total_actions += 1;
                    is_active = true;
                }

                let pressed_buttons = cmd.buttons & !last_cmds[i].buttons;
                if pressed_buttons != 0 {
                    let mut b = pressed_buttons;
                    while b > 0 {
                        players[i].total_actions += u32::from(b & 1);
                        b >>= 1;
                    }
                    is_active = true;
                } else if cmd.forward_move != 0
                    || cmd.side_move != 0
                    || cmd.angle_turn != 0
                    || cmd.buttons != 0
                {
                    is_active = true;
                }

                if is_active {
                    players[i].active_tics += 1;
                }

                last_cmds[i] = *cmd;
            }
        }

        for p in &mut players {
            if p.total_tics > 0 {
                let minutes = f64::from(p.total_tics) / (35.0 * 60.0);
                p.apm = f64::from(p.total_actions) / minutes;
            }
        }

        players
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apm_analyzer_core() {
        let mut tics = vec![];
        for _ in 0..35 {
            // 1 second of doing nothing
            tics.push(vec![DemoTicCmd::default()]);
        }
        // Then some action
        tics.push(vec![DemoTicCmd {
            forward_move: 50,
            side_move: 0,
            angle_turn: 0,
            buttons: 1,
        }]); // +2 actions
        tics.push(vec![DemoTicCmd {
            forward_move: 50,
            side_move: 0,
            angle_turn: 0,
            buttons: 0,
        }]); // +0 actions (held)
        tics.push(vec![DemoTicCmd {
            forward_move: 0,
            side_move: 0,
            angle_turn: 0,
            buttons: 0,
        }]); // +1 action (release forward)

        let stats = ApmAnalyzer::analyze_cmds(tics.into_iter());
        assert_eq!(stats.len(), 1);

        let p = &stats[0];
        assert_eq!(p.total_tics, 38);
        assert_eq!(p.total_actions, 3);
        assert_eq!(p.active_tics, 3);
        assert!(p.apm > 0.0);
    }
}
