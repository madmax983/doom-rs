🎯 Target: `doom-net` crate, specifically untested edge cases in `client.rs`, `server.rs`, `transport.rs`, `input_log.rs`, `rollback.rs`, and `snapshot.rs`.
💣 Risk: Various potential panic points and undefined behaviors due to missing bounds testing on socket operations, timeout pruning failures, missing coverage of packet edge scenarios, and `Option` dropping.
🧪 Strategy: Added robust polling loop unit tests and targeted edge-case evaluations across the network stack to correctly process or reject edge cases and bubble up `io::Error` instances on sockets gracefully. Fixed several `clippy::manual_assert` occurrences, replacing flaky assertions.
🔬 Verification: `cargo test -p doom-net`
