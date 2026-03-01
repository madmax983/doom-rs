//! Client-server rollback netcode.
//!
//! Wire format: `TicPacket` (bincode 2), CRC32 desync detection.
//! Verus-verified: snapshot ring coverage, input log contiguity.

pub mod client;
pub mod input_log;
pub mod packet;
pub mod rollback;
pub mod server;
pub mod transport;

pub use input_log::InputLog;
pub use packet::{TicPacket, WireTicCmd, MAX_PLAYERS};
pub use rollback::{MAX_ROLLBACK_TICS, SnapshotRing};

/// Errors produced by the netcode layer.
#[derive(Debug, thiserror::Error)]
pub enum NetError {
    /// Bincode decode failure (malformed or truncated packet).
    #[error("decode error: {0}")]
    Decode(String),

    /// Network I/O or protocol error.
    #[error("network error: {0}")]
    Network(String),
}
