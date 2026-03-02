//! Client-server rollback netcode for doom-rs.
//!
//! Wire format: [`TicPacket`] with manual little-endian serialization.
//! Desync detection via CRC32 checksums ([`checksum`] module).
//! Rollback: snapshot ring + input log + misprediction detection.
//!
//! This crate is fully synchronous -- no async, no tokio.  Actual UDP
//! transport will be layered on top by the application crate.
//!
//! [`TicCmd`] is defined locally as the wire-format player input command,
//! layout-compatible with `doom_game::TicCmd`.  doom-net depends only on
//! `doom-types`, not `doom-game`.

pub mod checksum;
pub mod input_log;
pub mod packet;
pub mod rollback;
pub mod snapshot;

pub use checksum::{CRC32_TABLE, checksums_match, compute_checksum};
pub use input_log::InputLog;
pub use packet::{MAX_PLAYERS, MAX_ROLLBACK_TICS, TIC_PACKET_SIZE, TicCmd, TicPacket};
pub use rollback::RollbackManager;
pub use snapshot::SnapshotRing;
