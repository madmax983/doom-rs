//! The Grand Assembly: Orchestrating the Doom Engine.
//!
//! The `doom-app` crate acts as the central hub that connects all the decoupled
//! subsystems (rendering, audio, networking, input, and game logic) into a single,
//! cohesive executable.
//!
//! # Responsibilities
//! - **Initialization:** Parsing CLI arguments, loading WAD files, and configuring
//!   the `doom-game` simulation.
//! - **Event Loop:** Pumping the terminal input, advancing the game state, and
//!   triggering the renderer at 35 Hz.
//! - **Subsystem Management:** Coordinating the `doom-audio` driver and `doom-net`
//!   client/server if applicable.
//! - **Demo Modes:** Wrapping the core event loop with recording or playback logic.
//!
//! This crate does not contain core game logic or rendering algorithms; it purely
//! drives the dependencies.
