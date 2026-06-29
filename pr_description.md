Title: ⚒️ Forge: Refactored boolean blindness in FaceState::tick

**🚮 Smell:**
The `FaceState::tick` function took multiple boolean arguments (`is_firing` and `is_invulnerable`), leading to Boolean Blindness and obscuring the intent at call sites (e.g., `face.tick(100, false, true, None)`).

**✨ Solution:**
Extracted these boolean arguments into strongly typed enums (`WeaponFiring` and `Invulnerability`) and updated the function signature and all call sites accordingly.

**🧼 Benefit:**
This refactoring significantly reduces cognitive load by making call sites self-documenting and enforces correct usage at compile time, eliminating the possibility of mixing up the boolean arguments.

**🛡️ Verification:**
Tests passed. No logic changed.
