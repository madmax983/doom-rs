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
//! `TicCmd` is defined locally as the wire-format player input command,
//! layout-compatible with `doom_game::TicCmd`.  doom-net depends only on
//! `doom-types`, not `doom-game`.

pub mod checksum;
pub mod client;
pub mod input_log;
pub mod packet;
pub mod rollback;
pub mod server;
pub mod snapshot;
pub mod transport;

pub use checksum::{CRC32_TABLE, checksums_match, compute_checksum};
pub use client::NetClient;
pub use input_log::InputLog;
pub use packet::{MAX_PLAYERS, MAX_ROLLBACK_TICS, TIC_PACKET_SIZE, TicPacket};
pub use rollback::RollbackManager;
pub use server::RelayServer;
pub use snapshot::SnapshotRing;
pub use transport::{ConnectionState, NetConfig, NetStats, NetTransport};
