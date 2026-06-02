//! Sector specials and linedef triggers.
//!
//! Port of Doom's `p_spec.c` and `p_ceilng.c` / `p_doors.c` (simplified).
//!
//! # Implemented
//! - `tick_sector_specials`: damage floors (specials 5, 7, 16) with periodic damage and RadSuit.
//! - `tick_sector_damage`: periodic damage (every 32 tics), RadSuit protection, God exit (special 11).
//! - `tick_doors`: advance active door/floor movers.
//! - `tick_lights`: advance light specials.
//! - `init_sector_lights`: create `SectorLightEffect` entries for sector specials 1-3, 8, 12-13, 17.
//! - `tick_sector_lights`: advance extended sector light effects.
//! - `spawn_level_specials`: initialise light thinkers on level load.
//! - `p_use_lines`: player USE activation, dispatches to `activate_linedef`.
//! - `activate_linedef`: doors, exits, crushers, lifts, floors, teleporters (39, 97, 125, 126).
//! - `ev_teleport`: teleport an actor to a teleport destination thing (kind 14).
//! - `player_sector_index`: find which sector the player is standing in.

pub mod activate;
pub mod ceilings;
pub mod constants;
pub mod doors;
pub mod floors;
pub mod lifts;
pub mod lights;
pub mod scroll;
pub mod sector;
pub mod teleport;
pub mod utils;

#[cfg(test)]
pub mod tests;

pub use activate::*;
pub use ceilings::*;
pub use constants::*;
pub use doors::*;
pub use floors::*;
pub use lifts::*;
pub use lights::*;
pub use scroll::*;
pub use sector::*;
pub use teleport::*;
pub use utils::*;
