//! Core logic and structures for the doom-app binary.
//!
//! This crate is primarily a binary (`src/main.rs`), but exposes a library interface
//! to allow other crates or integration tests to hook into the main event loop
//! or configuration parsing without spawning a subprocess.
