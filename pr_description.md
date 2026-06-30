⚒️ Forge: Refactored FaceState::tick to eliminate boolean blindness

🚰 Smell: `FaceState::tick` took multiple booleans (`is_firing`, `is_invulnerable`), obscuring intent at the call site.
✨ Solution: Introduced `WeaponFiring` and `Invulnerability` enums to strongly type these states.
🧼 Benefit: Improves readability and eliminates boolean blindness, enforcing correct usage at compile time.
🛡️ Verification: Tests passed. No logic changed.
