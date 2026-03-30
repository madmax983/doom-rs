//! Core numeric types and constants for the Doom engine.
//!
//! This crate is `no_std` compatible and contains zero OS-level dependencies.
//! All types here are candidates for Verus formal verification.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unexpected_cfgs)]

pub(crate) mod angle;
pub(crate) mod bbox;
pub(crate) mod fixed;
pub(crate) mod limits;
pub(crate) mod primitives;
pub(crate) mod vec2;

pub use angle::{ANG45, ANG90, ANG180, ANG270, Bam};
pub use bbox::BBox;
pub use fixed::{FIXED_ONE, FRAC_BITS, Fixed16_16};
pub use vec2::Vec2Fixed;

// Verus spine proofs (only processed by verus, not rustc).
#[allow(unexpected_cfgs)]
#[cfg(verus_keep_ghost)]
mod proofs;

pub use limits::*;
pub use primitives::*;
