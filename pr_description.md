🕸️ Tangle: `doom-game/src/player.rs` was unnecessarily re-exporting `doom_types::limits::{NUM_POWERS, NUM_PSPRITES}` via `pub use`, which violates boundary isolation by exposing shared limits directly from a downstream logic crate, which could cause external code to couple to `doom-game` rather than the foundational `doom-types` crate. In addition, there were clippy warnings for `set_skip` in `doom-tui`.

📐 Blueprint: Removed the `pub use` re-export from `doom-game/src/player.rs` and replaced it with a private `use`. Fixed the clippy deprecation warnings in `doom-tui` by wrapping `set_skip` with `#[allow(deprecated)]`.

🧱 Stability: Reduced coupling, and enforces cleaner dependency arrows.

🔬 Verification: Builds successfully, strict separation enforced. `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets --all-features` pass, and `cargo fmt --all` applied.
