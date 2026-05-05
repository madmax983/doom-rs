//! The main orchestrator and executable for the Doom Engine.
//!
//! `doom-app` acts as the conductor that ties together all of the independent subsystem
//! crates (`doom-game`, `doom-renderer`, `doom-audio`, `doom-tui`, etc.). It manages
//! the main execution loop, coordinates input polling, handles high-level configuration,
//! and passes state between the decoupled engine components.
//!
//! Because the underlying crates are highly decoupled and unaware of each other, `doom-app`
//! is responsible for bridging their boundaries.
