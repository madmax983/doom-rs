# ⚒️ Forge: Refactor manual double for-loops skipping first element

## 🚮 Smell
The manual double `for` loops spanning coordinates (x, y) used a mutable `bool` flag (like `past_first` or `skip_first`) to manually skip the first element in `crates/doom-tui/src/event_loop.rs` and `crates/doom-tui/src/sixel.rs`. This creates "boolean blindness" and visual noise.

Additionally, this logic was generating a deprecation warning on `Cell::set_skip`, which clutters the compilation output and causes `cargo clippy -D warnings` to fail.

## ✨ Solution
Flattened the Cartesian coordinate iteration into a `.flat_map().skip(1)` iterator chain and replaced `c.set_skip(true)` with `c.set_diff_option(ratatui::buffer::CellDiffOption::Skip)`.

## 🧼 Benefit
Eliminates boilerplate and statefulness related to the skipped boolean, making the iteration cleaner and declarative. Fixes the `ratatui` deprecation warning properly.

## 🛡️ Verification
Tests passed. No logic changed. `cargo clippy` is clean.
