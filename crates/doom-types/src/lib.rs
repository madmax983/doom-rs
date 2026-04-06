//! Core numeric types and constants for the Doom engine.
//!
//! This crate is `no_std` compatible and contains zero OS-level dependencies.
//! All types here are candidates for Verus formal verification.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unexpected_cfgs)]

pub mod angle;
pub mod bbox;
pub mod compat;
pub mod fixed;
pub mod limits;
pub mod primitives;
pub mod ticcmd;
pub mod vec2;

pub use angle::{ANG45, ANG90, ANG180, ANG270, Bam};
pub use bbox::BBox;
pub use compat::CompatibilityProfile;
pub use fixed::{FIXED_ONE, FRAC_BITS, Fixed16_16};
pub use ticcmd::{TicCmd, bt};
pub use vec2::Vec2Fixed;

// Verus spine proofs (only processed by verus, not rustc).
#[allow(unexpected_cfgs)]
#[cfg(verus_keep_ghost)]
mod proofs;
