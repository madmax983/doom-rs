#![allow(unused_imports)]
#![allow(dead_code)]
//! Sector specials and linedef triggers.

pub mod activation;
pub mod ceilings;
pub mod damage;
pub mod doors;
pub mod floors;
pub mod lifts;
pub mod lights;
pub mod platforms;
pub mod scroll;
pub mod stairs;
pub mod teleport;
pub mod util;

pub use activation::*;
pub use ceilings::*;
pub use damage::*;
pub use doors::*;
pub use floors::*;
pub use lifts::*;
pub use lights::*;
pub use platforms::*;
pub use scroll::*;
pub use stairs::*;
pub use teleport::*;
pub use util::*;
