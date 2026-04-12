1. **IDENTIFY:** The `player.rs` module manages `PlayerState` using primitive integer arithmetic for health, armor, and ammo. This is a prime target for integer overflow panics. The `actions.rs` module has a minor bug in the doctest due to a missing import or outdated enum usage.
2. **ATTACK:** Write a `proptest` harness in `player_tests.rs` to fuzz the mutating functions on `PlayerState` (`apply_damage`, `heal`, `give_armor`, `deduct_armor`, `give_ammo`, `use_ammo`, `set_health_capped`).
3. **DETONATE:** Run the proptest harness. It detonates immediately on:
   - `give_ammo`: `attempt to add with overflow`
   - `deduct_armor`: `attempt to subtract with overflow`
   - `set_health_capped`: `min > max` panic in `clamp`
4. **PRESENT:** Fix the vulnerabilities by using `.saturating_add()`, `.saturating_sub()`, and sanitizing bounds (`cap.max(0)`). Fix the doctest in `actions.rs`. Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done. Submit the PR.
