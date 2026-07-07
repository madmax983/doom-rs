# ⚒️ Forge: Refactor Boolean Blindness in next_map

**🚮 Smell:** The `next_map(secret_exit: bool)` function suffered from Boolean Blindness, hiding the intent of `true`/`false` at call sites.
**✨ Solution:** Replaced the `secret_exit: bool` parameter with a strongly typed `ExitType` enum (`Normal`, `Secret`). Updated `next_map_doom1`, `next_map_doom2`, and all call sites in `phase.rs`.
**🧼 Benefit:** Improves readability and clearly documents the intent at the call site, enforcing correct usage via types.
**🛡️ Verification:** Ran `cargo clippy`, `cargo fmt`, and `cargo test`. All passed. No logic changed.
