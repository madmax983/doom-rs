//! The Grand Assembly: Orchestrating the Doom Engine.
//!
//! While the other crates in this workspace are specialized components (rendering,
//! audio, game logic), `doom-app` is the conductor that brings them all together.
//! It initializes the windowing system, sets up the audio driver, parses command-line
//! arguments, and runs the main simulation loop.
//!
//! This is the entry point where the isolated state machines of `doom-game` meet
//! the real world of OS events, user input, and screen updates.
