//! Cogmind-mode rendering: top-down ASCII roguelike view of Doom maps.
//!
//! Items here are consumed by later tasks in the cogmind feature pipeline;
//! suppress dead-code warnings until the rendering pipeline is wired up.
#![allow(dead_code)]

pub mod glyphs;
pub mod tile_grid;
pub mod visibility;
