//! Client-server rollback netcode for doom-rs.
//!
//! Wire format: [`TicPacket`] with manual little-endian serialization.
//! Desync detection via CRC32 checksums ([`checksum`] module).
//! Rollback: snapshot ring + input log + misprediction detection.
//! UDP transport: [`transport::NetTransport`], [`server::RelayServer`],
//! [`client::NetClient`].
//!
//! This crate is fully synchronous -- no async, no tokio.  UDP transport
//! uses `std::net::UdpSocket` in non-blocking mode.
//!
//! [`doom_types::TicCmd`] is the wire-format player input command. It is
//! imported from `doom-types` so that it can be shared across the workspace.
//! `doom-net` depends only on `doom-types`, not `doom-game`.

/// State checksumming.
pub mod checksum;
/// Network client.
///
/// Connects to a server and handles packets.
pub mod client;
/// Input logging and replay.
pub mod input_log;
/// Network packet structures.
///
/// Tic commands and other data packets.
pub mod packet;
/// Rollback netcode management.
pub mod rollback;
/// Network relay server.
pub mod server;
/// State snapshotting.
pub mod snapshot;
/// Network transport layer.
pub mod transport;

pub use checksum::{CRC32_TABLE, checksums_match, compute_checksum};
pub use client::NetClient;
pub use input_log::InputLog;
pub use packet::{MAX_PLAYERS, MAX_ROLLBACK_TICS, TIC_PACKET_SIZE, TicPacket};
pub use rollback::RollbackManager;
pub use server::RelayServer;
pub use snapshot::SnapshotRing;
pub use transport::{ConnectionState, NetConfig, NetStats, NetTransport};
