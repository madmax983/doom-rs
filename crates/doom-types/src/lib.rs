//! Core numeric types and constants for the Doom engine.
//!
//! This crate is `no_std` compatible and contains zero OS-level dependencies.
//! All types here are candidates for Verus formal verification.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod angle;
pub mod bbox;
pub mod fixed;
pub mod limits;
pub mod primitives;
pub mod vec2;

pub use angle::{Bam, ANG45, ANG90, ANG180, ANG270};
pub use bbox::BBox;
pub use fixed::{Fixed16_16, FIXED_ONE, FRAC_BITS};
pub use vec2::Vec2Fixed;

// Verus spine proofs (only processed by verus, not rustc).
#[cfg(verus_keep_ghost)]
mod proofs;
