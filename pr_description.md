🎯 Target: Fixed flaky tests in `crates/doom-tui/src/event_loop.rs` by correctly adding Mutex locking and added test coverage for gaps in `crates/doom-types` modules (`angle.rs`, `fixed.rs`, `bbox.rs`).
💣 Risk: Flaky tests on event loop could sporadically cause CI failures. Missing test coverage for core primitives (`Default` traits and conversion traits) hides untested behavior.
🧪 Strategy: Acquired `MODIFIER_COUNT_LOCK` across previously concurrent unlocked tests in `event_loop.rs` preventing random poison errors. Added tests validating edge cases for `Default` and `From` trait implementations across `Fixed16_16`, `Bam`, and `BBox` types.
🔬 Verification: `cargo test -p doom-types` and `cargo test -p doom-tui` pass correctly.
