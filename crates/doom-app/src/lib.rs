//! Library components for the Doom engine application orchestrator.
//!
//! This crate contains modules that link together the various game subsystems
//! (like audio, rendering, networking, and input) to drive the main execution loop.
//! While `main.rs` serves as the primary entry point and orchestrator, this library
//! exposes the components required to manage the game's lifecycle, savegames,
//! demo recording/playback, and input multiplexing.

// We don't export modules here if they are already integrated into main.rs
// to avoid duplicate definitions and test failures.
