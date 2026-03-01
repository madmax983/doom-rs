//! Client-server rollback netcode.
//!
//! Wire format: `TicPacket` (bincode), CRC32 desync detection.
//! Verus-verified: snapshot ring coverage, input log contiguity.

pub mod client;
pub mod input_log;
pub mod packet;
pub mod rollback;
pub mod server;
pub mod transport;
