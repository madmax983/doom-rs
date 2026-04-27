🗺️ Atlas: Make internal modules of doom-game pub(crate) to enforce boundaries

🕸️ Tangle: doom-game's internal modules were publicly exported, leading to downstream crates depending on internal implementation details and creating a leaky abstraction.
📐 Blueprint: Changed internal mod declarations in doom-game to pub(crate), and updated all downstream dependents (doom-app, doom-renderer, doom-tui) to use the correct exported symbols from the root of doom-game instead of reaching into internal modules.
🧱 Stability: Reduces coupling across crates and enforces the root of doom-game as a true facade.
🔬 Verification: Builds successfully and passes all tests and clippy checks without regressions.
