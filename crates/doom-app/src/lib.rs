//! The Grand Assembly Library.
//!
//! While `main.rs` holds the orchestrator loop, this library module
//! houses the core components that tie the disparate parts of the Doom Engine
//! together, such as configuration parsing, cheats management, savegame serialization,
//! and the abstract event loop for running different modes (single-player,
//! demo playback, and multiplayer).
//!
//! # Modes of Play
//!
//! The orchestration delegates specific loops to modular game loops depending on context:
//! - `` `demo_mode` ``: Handles reading and replaying LMP files as simulated user input.
//! - `` `net_mode` ``: Translates UDP network events into deterministic TicCommands.
