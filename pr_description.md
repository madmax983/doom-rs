🚮 Smell: `FaceState::tick` took multiple consecutive boolean arguments (`is_firing: bool`, `is_invulnerable: bool`), causing "boolean blindness" at call sites like `face.tick(100, true, false, None)`.
✨ Solution: Replaced raw booleans with strict enums `PlayerFiringStatus` and `PlayerInvulnerability`.
🧼 Benefit: Call sites are now self-documenting and type-safe, preventing accidental parameter swapping.
🛡️ Verification: Tests passed. No logic changed.
