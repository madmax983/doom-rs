🚮 Smell: Boolean Blindness in map progression logic. The `next_map` method and its internal helpers (`next_map_doom1` and `next_map_doom2`) in `crates/doom-game/src/phase.rs` accepted a boolean argument (`secret_exit: bool`), which obscured intent at call sites (`next_map(true)` vs `next_map(false)`).

✨ Solution: Reused the existing, strongly-typed `ExitRequest` enum (containing variants like `Normal` and `Secret`) and refactored `next_map` and its callers to use this enum instead of a boolean value.

🧼 Benefit: Dramatically improves readability, makes the intent at call sites clear without needing to jump to the method definition, and leverages Rust's type system to prevent mixing up boolean parameters.

🛡️ Verification: All 87 unit tests in `phase.rs` were successfully updated and `cargo test -p doom-game --lib phase` executed, resulting in all tests passing. Furthermore, workspace-wide validations (`cargo check --workspace` and `cargo clippy`) confirmed the absence of logic breakage and out-of-scope bugs. No runtime behavior was changed.
