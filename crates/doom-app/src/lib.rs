//! Doom engine application wrapper.
//!
//! # Overview
//!
//! `doom-app` acts as the orchestrator for the various decoupled subsystems, such as
//! `doom-game`, `doom-renderer`, `doom-audio`, and `doom-tui`. It owns the main executable
//! loop and wires the isolated systems together.
//!
//! # Core Responsibilities
//!
//! - Orchestrating the game loop
//! - Managing different modes of play (e.g., singleplayer, demo playback)
