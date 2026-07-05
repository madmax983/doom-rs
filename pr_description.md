🚮 Smell: Boolean Blindness in `hitscan_shot_angle` taking a `bool` flag for `accurate_first_shot`. Call sites like `hitscan_shot_angle(gs, autoaim_angle, true)` are opaque.
✨ Solution: Extracted the boolean into a strictly typed `HitscanAccuracy` enum (`AccurateFirstShot`, `AlwaysSpread`).
🧼 Benefit: Self-documents the behavior at call sites (`HitscanAccuracy::AccurateFirstShot`) and prevents boolean blindness.
🛡️ Verification: Tests passed. No logic changed.
