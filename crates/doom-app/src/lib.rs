//! The core application orchestration for Doom.
//!
//! This crate wires together the engine's decoupled systems (renderer, audio, game logic,
//! networking, and tui) into the "Grand Assembly" that forms the actual playable game.
//! It handles the main event loop, system initialization, and modes of operation
//! like demo recording/playback, netplay, and standard execution.
