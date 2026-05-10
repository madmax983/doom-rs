//! The Doom engine entry point and orchestrator.
//!
//! While the `doom-*` crates are meticulously decoupled components—`doom-game`
//! simulates the world, `doom-renderer` paints the walls, `doom-audio` mixes the
//! screams, and `doom-tui` maps pixels to the terminal—they cannot play Doom
//! on their own. They need a conductor.
//!
//! `doom-app` is that conductor. It owns the main executable loop and wires
//! the isolated systems together. It parses command-line arguments, loads the
//! `WadFile`s, initializes the `AudioSystem`, sets up the `Terminal`, and
//! pumps the `DoomEventLoop`.
//!
//! # Modes of Play
//!
//! The app can boot into several different modes depending on the arguments:
//! - **Singleplayer**: The default mode. Connects local `TicInput` directly to `doom-game`.
//! - **Demo Playback**: Wraps the game in a `demo_mode::DemoPlaybackApp`, ignoring
//!   local input and feeding pre-recorded tics from an LMP file.
//! - **Demo Recording**: Wraps the game in a `demo_mode::DemoRecordingWrapper`, saving
//!   every local input to disk while playing.
//! - **Netplay Client**: Uses `net_mode::NetGameApp` to synchronize tics over UDP
//!   with a relay server before feeding them to the local simulation.
