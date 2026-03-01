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
pub mod combat;
pub mod dehacked;
pub mod mobj;
pub mod mobjinfo;
pub mod movement;
pub mod player;
pub mod snapshot;
pub mod specials;
pub mod state;
pub mod states;
pub mod pickups;
pub mod tic;
pub mod weapons;

pub use actions::{ACTION_CHASE, ACTION_LOOK, ACTION_NONE, dispatch_action};
pub use dehacked::{DehError, DehPatch, FramePatch, ThingPatch, WeaponPatch};
pub use combat::{MELEERANGE, MISSILERANGE, damage_mobj, p_line_attack, p_radius_attack};
pub use mobj::{Mobj, MobjHandle, MobjKind, MobjSlab, StateNum, flags};
pub use mobjinfo::{MobjInfo, MOBJINFO};
pub use movement::{MAX_STEP_HEIGHT, p_try_move};
pub use player::{AmmoType, PlayerState, WeaponType, WEAPON_AMMO};
pub use snapshot::Snapshot;
pub use specials::{USE_RANGE, activate_linedef, p_use_lines, tick_sector_specials};
pub use state::{DoomRng, GameState, RNG_TABLE};
pub use states::STATES;
pub use tic::{FRICTION, MAXMOVE, PLAYER_SPEED_SCALE, TicCmd, bt};
pub use pickups::p_check_pickups;
pub use weapons::{fire_weapon, player_can_fire};
