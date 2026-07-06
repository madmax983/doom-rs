🗺️ Atlas: Unify PaletteFlash Tracking

🕸️ Tangle: The codebase had two separate structures for palette flashing: `PaletteFlash` (a simple timer-based trigger) and `PaletteFlashState` (a robust multi-source priority tracker). `doom-app` was using the simpler `PaletteFlash`, requiring manual calculation of damage-to-palette conversions and hardcoded overrides, leading to poor encapsulation and redundant types.
📐 Blueprint: Removed `PaletteFlash` entirely and migrated `doom-app` to use `PaletteFlashState`. The game loop now correctly delegates state tracking (pain, bonus, radsuit, berserk) to `PaletteFlashState`, which handles priority natively. This simplifies the top-level loop and enforces the single-responsibility principle.
🧱 Stability: Removed duplicate state management, correctly tracks priority of concurrent screen flashes, and reduces coupling to the rendering layer.
🔬 Verification: Unit tests for `doom-app` updated and passing. `cargo test --workspace` passes, and the codebase compiles with `cargo clippy --all-targets --all-features -- -D warnings`.
