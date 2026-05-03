//! The Grand Orchestrator for the Doom engine.
//!
//! This crate serves as the master assembly point that wires together all the decoupled
//! systems of the engine (`doom-game`, `doom-renderer`, `doom-audio`, `doom-net`, etc.).
//! It defines the primary application loops, coordinates data passing between the core
//! simulation and the output backends, and handles varying execution modes like single-player,
//! demo playback, and multiplayer.
