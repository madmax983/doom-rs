# Doom Engine in Rust

A modular, terminal-based implementation of the classic Doom engine, written in Rust.

## Architecture

The engine is divided into several decoupled crates to ensure separation of concerns and maintainability.

- **`doom-types`**: Core numeric types, math primitives, and bounded values. Zero dependencies, `no_std` compatible, and intended for formal verification.
- **`doom-wad`**: WAD file parsing, lump management, and asset extraction.
- **`doom-map`**: BSP tree traversal, classic binary map parsing, Doom-namespace UDMF `TEXTMAP` parsing, and subsector management.
- **`doom-game`**: The core game logic, including the state machine, player movement, entity interactions, weapons, and collision detection. Completely independent of rendering and audio.
- **`doom-audio`**: Audio backend managing SFX mixing and OPL2 MIDI playback via `cpal`.
- **`doom-net`**: Multiplayer networking logic using UDP sockets, providing client and server implementations for Tic command synchronization.
- **`doom-renderer`**: Software renderer that draws walls, flats, and sprites into an off-screen buffer.
- **`doom-tui`**: Terminal User Interface handling input and displaying the rendered frame using `ratatui` and `crossterm`.
- **`doom-demo`**: Demo recording and playback functionality.
- **`doom-app`**: The main executable that orchestrates all subsystems, handling the main loop, input polling, and configuration.

## Development

- Requires `libasound2-dev` on Linux for audio support.
- Use `cargo run --release` to run the game (it requires a WAD file).
- Use `cargo doc --open` to explore the extensive internal documentation.
