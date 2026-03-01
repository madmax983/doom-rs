//! Doom game simulation: thinker loop, MOBJ slab arena, player, monsters,
//! combat, sector specials, and rollback snapshots.
//!
//! # Architecture
//! - `GameState` owns `MobjSlab` + `PlayerState` + deterministic RNG.
//! - `tick(TicCmd)` is the single entry point for one simulation step.
//! - `save_snapshot` / `restore_snapshot` provide deep-copy rollback.
//!
//! # Invariants (Verus-verifiable)
//! - `player.health ≤ MAX_HEALTH` at all times.
//! - `player.ammo[i] ≤ MAX_AMMO[i]` for all i.
//! - Dead actors (`health ≤ 0`) never transition to attack states (batch 2).

pub mod actions;
pub mod mobj;
pub mod mobjinfo;
pub mod movement;
pub mod player;
pub mod snapshot;
pub mod state;
pub mod states;
pub mod tic;

pub use actions::{ACTION_CHASE, ACTION_LOOK, ACTION_NONE, dispatch_action};
pub use mobj::{Mobj, MobjHandle, MobjKind, MobjSlab, StateNum, flags};
pub use mobjinfo::{MobjInfo, MOBJINFO};
pub use movement::{MAX_STEP_HEIGHT, p_try_move};
pub use player::{AmmoType, PlayerState, WeaponType, WEAPON_AMMO};
pub use snapshot::Snapshot;
pub use state::{DoomRng, GameState, RNG_TABLE};
pub use states::STATES;
pub use tic::{FRICTION, MAXMOVE, PLAYER_SPEED_SCALE, TicCmd, bt};
