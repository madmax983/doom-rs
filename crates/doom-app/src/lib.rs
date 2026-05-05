//! Doom engine orchestrator and CLI entry point.
//!
//! This crate is the top-level application binary for the `doom-rs` workspace.
//! It serves as the master orchestrator, wiring together the highly decoupled
//! underlying `doom-*` crates:
//!
//! - **`doom-types`**: Core mathematical and bounded numeric types.
//! - **`doom-wad`**: WAD archive parsing and asset extraction.
//! - **`doom-map`**: BSP tree traversal and level topology parsing.
//! - **`doom-game`**: The core simulation and state machine.
//! - **`doom-renderer`**: The software renderer that draws the 3D world.
//! - **`doom-audio`**: The OPL2 and SFX mixing audio backend.
//! - **`doom-tui`**: The terminal user interface for rendering graphics.
//! - **`doom-net`**: Multiplayer UDP synchronization.
//! - **`doom-demo`**: LMP demo recording and playback.
//!
//! `doom-app` provides the primary event loop, parses command-line arguments
//! using `clap`, handles input collection, and wires all subsystems together
//! to form a playable game.
