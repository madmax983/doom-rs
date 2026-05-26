//! Main application crate orchestrating the Doom engine.
//!
//! This crate wires up the game logic (`doom-game`), rendering (`doom-renderer`),
//! audio (`doom-audio`), and input (`doom-tui`) into a cohesive application loop.
//! It is typically run via the `main.rs` binary entry point.
//!
//! The primary structure here is the game loop, which pumps input into the state
//! machine, ticks the game state, and hands the resulting structures to the renderer.
