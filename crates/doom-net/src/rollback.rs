//! Rollback manager for client-side prediction and correction.
//!
//! [`RollbackManager`] coordinates the snapshot ring, input log, and
//! rollback decision logic.  It does NOT own the game state or run the
//! simulation -- the caller is responsible for actually replaying tics
//! when `needs_rollback()` returns `Some(tic)`.

use crate::input_log::InputLog;
use crate::packet::{MAX_PLAYERS, MAX_ROLLBACK_TICS, TicPacket};
use doom_types::TicCmd;
use crate::snapshot::SnapshotRing;

// ---------------------------------------------------------------------------
// RollbackManager
// ---------------------------------------------------------------------------

/// Coordinates snapshots, input logs, and rollback detection.
///
/// Generic over `S: Clone` so that the game state type does not need to
/// be imported from `doom-game` at the type level (only at call sites).
///
/// ## Examples
///
/// ```rust
/// use doom_net::RollbackManager;
/// use doom_net::{TicCmd, TicPacket, packet::MAX_PLAYERS};
///
/// // Game state can be any Clone type.
/// #[derive(Clone, Debug, PartialEq)]
/// struct GameState { hp: i32 }
///
/// // Create a manager for player slot 0.
/// let mut rm: RollbackManager<GameState> = RollbackManager::new(0);
///
/// // 1. Save a snapshot of the state at tic 0.
/// rm.save_snapshot(0, GameState { hp: 100 });
///
/// // 2. Predict local input for tic 0.
/// let predicted_cmd = TicCmd { forward_move: 50, ..TicCmd::default() };
/// rm.record_local_input(0, predicted_cmd);
///
/// // 3. Simulate tic 0 locally (game loop would do this).
/// rm.advance_tic(); // now at tic 1
///
/// // 4. Server authoritative packet arrives for tic 0, but contradicts our prediction!
/// let mut auth_cmds = [TicCmd::default(); MAX_PLAYERS];
/// auth_cmds[0] = TicCmd { forward_move: -50, ..TicCmd::default() }; // They moved backwards!
///
/// let auth_packet = TicPacket {
///     tic: 0,
///     sender: 255, // Server
///     ack_tic: 0,
///     state_checksum: 0,
///     cmds: auth_cmds,
/// };
/// rm.receive_packet(&auth_packet);
///
/// // 5. The manager detects the mismatch and flags a rollback.
/// assert_eq!(rm.needs_rollback(), Some(0));
/// ```
#[derive(Debug, Clone)]
pub struct RollbackManager<S: Clone> {
    /// Snapshot ring for game state rollback.
    snapshots: SnapshotRing<S>,
    /// Per-tic input history (predicted + authoritative).
    input_log: InputLog,
    /// This client's player slot (0-based).
    local_player: u8,
    /// The tic that will be simulated next.
    current_tic: u32,
    /// Latest tic for which we have authoritative (server-confirmed) inputs.
    confirmed_tic: u32,
    /// Tic at which a misprediction was detected.  The caller should
    /// rollback to this tic and replay forward.  Reset to `None` after
    /// the caller acknowledges it.
    rollback_tic: Option<u32>,
}

impl<S: Clone> RollbackManager<S> {
    /// Create a new manager for `local_player`, starting at tic 0.
    #[must_use]
    pub fn new(local_player: u8) -> Self {
        Self {
            snapshots: SnapshotRing::new(MAX_ROLLBACK_TICS),
            input_log: InputLog::new(MAX_ROLLBACK_TICS * 2),
            local_player,
            current_tic: 0,
            confirmed_tic: 0,
            rollback_tic: None,
        }
    }

    /// Save a snapshot of the game state at `tic`.
    pub fn save_snapshot(&mut self, tic: u32, state: S) {
        self.snapshots.save(tic, state);
    }

    /// Retrieve the snapshot for `tic`, if still in the ring.
    #[must_use]
    pub fn get_snapshot(&self, tic: u32) -> Option<&S> {
        self.snapshots.get(tic)
    }

    /// Record the local player's input for `tic`.
    ///
    /// The command is placed into the local player's slot; remote player
    /// slots are left as default (zeroed) until authoritative data arrives.
    pub fn record_local_input(&mut self, tic: u32, cmd: TicCmd) {
        // Build a cmds array with the local player's input in the right slot.
        let mut cmds = self
            .input_log
            .get(tic)
            .map_or_else(|| [TicCmd::default(); MAX_PLAYERS], |existing| *existing);
        let slot = (self.local_player as usize) % MAX_PLAYERS;
        cmds[slot] = cmd;
        self.input_log.record(tic, cmds);
    }

    /// Process an authoritative packet from the server.
    ///
    /// Stores the server-confirmed inputs and updates `confirmed_tic`.
    /// If the authoritative inputs differ from our predicted inputs for
    /// that tic, a rollback is flagged.
    pub fn receive_packet(&mut self, packet: &TicPacket) {
        let tic = packet.tic;

        // Check if predicted inputs differ from authoritative.
        let mispredicted = self
            .input_log
            .get(tic)
            .is_some_and(|predicted| *predicted != packet.cmds);

        // Store the authoritative inputs.
        self.input_log.set_authoritative(tic, packet.cmds);

        // Advance confirmed_tic.
        if tic >= self.confirmed_tic {
            self.confirmed_tic = tic;
        }

        // Flag rollback if mispredicted and within the rollback window.
        if mispredicted {
            match self.rollback_tic {
                Some(existing) if existing <= tic => {} // already have an earlier rollback pending
                _ => self.rollback_tic = Some(tic),
            }
        }
    }

    /// If a rollback is needed, returns the tic to roll back to and clears
    /// the flag.  Returns `None` if no rollback is pending.
    pub const fn needs_rollback(&mut self) -> Option<u32> {
        self.rollback_tic.take()
    }

    /// Retrieves the recorded player inputs for the specified game `tic`, utilized to replay history during state rollbacks.
    ///
    /// Returns authoritative inputs if available, otherwise predicts by
    /// repeating the last known input for each player.
    #[must_use]
    pub fn get_inputs(&self, tic: u32) -> [TicCmd; MAX_PLAYERS] {
        // If we have authoritative (or any recorded) inputs for this tic, use them.
        if let Some(cmds) = self.input_log.get(tic) {
            return *cmds;
        }

        // Predict: search backwards for the most recent recorded input.
        let search_depth = MAX_ROLLBACK_TICS as u32;
        for delta in 1..=search_depth {
            if let Some(prev_tic) = tic.checked_sub(delta) {
                if let Some(cmds) = self.input_log.get(prev_tic) {
                    return *cmds;
                }
            }
        }

        // No history at all -- return defaults.
        [TicCmd::default(); MAX_PLAYERS]
    }

    /// Advance to the next tic.
    pub const fn advance_tic(&mut self) {
        self.current_tic = self.current_tic.wrapping_add(1);
    }

    /// The tic that will be simulated next.
    #[must_use]
    pub const fn current_tic(&self) -> u32 {
        self.current_tic
    }

    /// The latest tic with server-confirmed inputs.
    #[must_use]
    pub const fn confirmed_tic(&self) -> u32 {
        self.confirmed_tic
    }

    /// This client's player slot.
    #[must_use]
    pub const fn local_player(&self) -> u8 {
        self.local_player
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(forward: i8) -> TicCmd {
        TicCmd {
            forward_move: forward,
            ..TicCmd::default()
        }
    }

    fn make_auth_packet(tic: u32, cmds: [TicCmd; MAX_PLAYERS]) -> TicPacket {
        TicPacket {
            tic,
            sender: 255, // server
            ack_tic: tic,
            state_checksum: 0,
            cmds,
        }
    }

    #[test]
    fn new_initializes_at_tic_zero() {
        let rm: RollbackManager<u32> = RollbackManager::new(0);
        assert_eq!(rm.current_tic(), 0);
        assert_eq!(rm.confirmed_tic(), 0);
        assert_eq!(rm.local_player(), 0);
    }

    #[test]
    fn save_snapshot_and_retrieve() {
        let mut rm: RollbackManager<String> = RollbackManager::new(0);
        rm.save_snapshot(5, "state_at_5".to_string());
        assert_eq!(rm.get_snapshot(5), Some(&"state_at_5".to_string()));
        assert!(rm.get_snapshot(6).is_none());
    }

    #[test]
    fn save_snapshot_receive_packet_flow() {
        let mut rm: RollbackManager<u64> = RollbackManager::new(0);
        rm.save_snapshot(0, 1000);

        let mut cmds = [TicCmd::default(); MAX_PLAYERS];
        cmds[0] = cmd(50);
        let pkt = make_auth_packet(0, cmds);
        rm.receive_packet(&pkt);

        assert_eq!(rm.confirmed_tic(), 0);
        assert!(rm.get_snapshot(0).is_some());
    }

    #[test]
    fn needs_rollback_none_when_no_auth_data() {
        let mut rm: RollbackManager<u32> = RollbackManager::new(0);
        assert!(
            rm.needs_rollback().is_none(),
            "no rollback needed when no authoritative data received"
        );
    }

    #[test]
    fn needs_rollback_some_when_mispredicted() {
        let mut rm: RollbackManager<u32> = RollbackManager::new(0);

        // Record a local prediction for tic 0.
        rm.record_local_input(0, cmd(50));

        // Server says player 0 actually had forward=99 -- different from our prediction.
        let mut auth_cmds = [TicCmd::default(); MAX_PLAYERS];
        auth_cmds[0] = cmd(99);
        let pkt = make_auth_packet(0, auth_cmds);
        rm.receive_packet(&pkt);

        let rb = rm.needs_rollback();
        assert_eq!(
            rb,
            Some(0),
            "must flag rollback when authoritative differs from predicted"
        );
    }

    #[test]
    fn needs_rollback_none_when_prediction_matches() {
        let mut rm: RollbackManager<u32> = RollbackManager::new(0);

        let mut cmds = [TicCmd::default(); MAX_PLAYERS];
        cmds[0] = cmd(50);
        rm.record_local_input(0, cmd(50));

        // Server confirms our prediction exactly.
        let pkt = make_auth_packet(0, cmds);
        rm.receive_packet(&pkt);

        assert!(
            rm.needs_rollback().is_none(),
            "no rollback when prediction matches authoritative"
        );
    }

    #[test]
    fn get_inputs_returns_authoritative_when_available() {
        let mut rm: RollbackManager<u32> = RollbackManager::new(0);

        let mut auth_cmds = [TicCmd::default(); MAX_PLAYERS];
        auth_cmds[0] = cmd(77);
        let pkt = make_auth_packet(3, auth_cmds);
        rm.receive_packet(&pkt);

        let inputs = rm.get_inputs(3);
        assert_eq!(
            inputs[0].forward_move, 77,
            "get_inputs must return authoritative data"
        );
    }

    #[test]
    fn get_inputs_predicts_repeat_last_known() {
        let mut rm: RollbackManager<u32> = RollbackManager::new(0);

        // Record input for tic 5.
        rm.record_local_input(5, cmd(33));

        // Ask for tic 6 -- no data recorded, so it should repeat tic 5.
        let inputs = rm.get_inputs(6);
        assert_eq!(
            inputs[0].forward_move, 33,
            "prediction must repeat last known input"
        );
    }

    #[test]
    fn get_inputs_returns_default_when_no_history() {
        let rm: RollbackManager<u32> = RollbackManager::new(0);
        let inputs = rm.get_inputs(100);
        assert_eq!(
            inputs,
            [TicCmd::default(); MAX_PLAYERS],
            "no history must return defaults"
        );
    }

    #[test]
    fn advance_tic_increments() {
        let mut rm: RollbackManager<u32> = RollbackManager::new(0);
        assert_eq!(rm.current_tic(), 0);
        rm.advance_tic();
        assert_eq!(rm.current_tic(), 1);
        rm.advance_tic();
        assert_eq!(rm.current_tic(), 2);
    }

    #[test]
    fn record_local_input_places_cmd_in_correct_slot() {
        let mut rm: RollbackManager<u32> = RollbackManager::new(2); // player 2
        rm.record_local_input(0, cmd(42));

        let inputs = rm.get_inputs(0);
        // Player 2's slot should have forward_move=42.
        assert_eq!(
            inputs[2].forward_move, 42,
            "local input must be in the local_player slot"
        );
        // Other slots should be default.
        assert_eq!(inputs[0].forward_move, 0);
        assert_eq!(inputs[1].forward_move, 0);
        assert_eq!(inputs[3].forward_move, 0);
    }

    #[test]
    fn confirmed_tic_advances_with_packets() {
        let mut rm: RollbackManager<u32> = RollbackManager::new(0);
        assert_eq!(rm.confirmed_tic(), 0);

        let pkt = make_auth_packet(5, [TicCmd::default(); MAX_PLAYERS]);
        rm.receive_packet(&pkt);
        assert_eq!(rm.confirmed_tic(), 5);

        let pkt2 = make_auth_packet(7, [TicCmd::default(); MAX_PLAYERS]);
        rm.receive_packet(&pkt2);
        assert_eq!(rm.confirmed_tic(), 7);
    }

    #[test]
    fn needs_rollback_clears_after_read() {
        let mut rm: RollbackManager<u32> = RollbackManager::new(0);
        rm.record_local_input(0, cmd(10));

        let mut auth = [TicCmd::default(); MAX_PLAYERS];
        auth[0] = cmd(99);
        rm.receive_packet(&make_auth_packet(0, auth));

        assert!(rm.needs_rollback().is_some(), "first call returns Some");
        assert!(
            rm.needs_rollback().is_none(),
            "second call returns None (flag cleared)"
        );
    }

    #[test]
    fn rollback_picks_earliest_misprediction() {
        let mut rm: RollbackManager<u32> = RollbackManager::new(0);

        rm.record_local_input(3, cmd(10));
        rm.record_local_input(5, cmd(20));

        // Misprediction at tic 5 first...
        let mut auth5 = [TicCmd::default(); MAX_PLAYERS];
        auth5[0] = cmd(99);
        rm.receive_packet(&make_auth_packet(5, auth5));

        // ...then misprediction at tic 3 (earlier).
        let mut auth3 = [TicCmd::default(); MAX_PLAYERS];
        auth3[0] = cmd(88);
        rm.receive_packet(&make_auth_packet(3, auth3));

        let rb = rm.needs_rollback();
        assert_eq!(
            rb,
            Some(3),
            "rollback must point to the earliest misprediction"
        );
    }

    #[test]
    fn get_inputs_predicts_stops_at_tic_0() {
        let mut rm: RollbackManager<u32> = RollbackManager::new(0);

        // Record input for tic 0.
        rm.record_local_input(0, cmd(45));

        // Ask for tic 5 (depth max is 8).
        // It will search back to tic 0 and stop because 0 - 6 would underflow.
        let inputs = rm.get_inputs(5);
        assert_eq!(
            inputs[0].forward_move, 45,
            "prediction must find tic 0 and correctly avoid underflow"
        );

        let inputs2 = rm.get_inputs(10);
        assert_eq!(
            inputs2[0].forward_move, 0,
            "prediction should fail to find history beyond max depth"
        );
    }
}
