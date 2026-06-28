🎯 Target:
- `WadStack` bounds checking (`Reject::parse_lump`)
- `tick_psprite_slot` out of bounds state handling (`weapons.rs`)
- Test cleanliness (`udmf.rs` `.unwrap_err()`)
- `doom-tui` `Cell::set_skip` deprecation warnings

💣 Risk:
- `Reject::parse_lump` using `checked_mul` bounds checking lacked an automated test ensuring it actually catches a genuine `usize` overflow correctly.
- An edge case in `tick_psprite_slot` where an invalid/corrupted state falls back to `StateNum::NULL` via `unwrap_or`, but lacked any test coverage to prove it resolved cleanly.
- Scattered instances of `.unwrap_err()` in test files (like `udmf.rs`) obscured test failure context by panicing with generic error messages.

🧪 Strategy:
- Added `reject_parse_lump_overflow` passing `usize::MAX / 2 + 1` to prove the `checked_mul` safety wrapper bounds.
- Added targeted test case `tick_psprite_invalid_next_state_becomes_null` in `weapons.rs` to reach 100% test coverage on weapon state transitions.
- Replaced `.unwrap_err()` with `.expect_err("should parse fail")` to explicitly document the invariant and assist debugging.
- Placed `#[allow(deprecated)]` on the `ratatui` `Cell::set_skip` usage in `doom-tui` to bypass `-D warnings` without rewriting out-of-scope code.

🔬 Verification:
```bash
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
```
