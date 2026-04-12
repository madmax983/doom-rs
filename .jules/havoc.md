## 2024-05-18 - [Havoc: OOM on TEXTURE1 parser]
**Learning:** Doom's TEXTURE1 parsing uses a direct 4-byte `num_textures` read to allocate `Vec::with_capacity(num_textures)`. Fuzzing this length with large values triggers an immediate OOM.
**Action:** Use `.min(data.len() / 4)` to clamp lengths derived from WAD/lump headers, preventing massive allocations while still ensuring we parse valid entries up to the slice boundary.
**Fuzzing Integer Overflows in Player Math**
**Learning:** Found multiple hidden panics (`attempt to add with overflow`, `attempt to subtract with overflow`, `min > max`) in standard `player.rs` functions (`give_ammo`, `deduct_armor`, `set_health_capped`) when fuzzing them with edge-case `i32` and `u32` integers using `proptest`.
**Action:** When performing chaos testing on data structures managing player health, armor, or ammo, always write a `proptest` harness specifically targeting bounds checks and arithmetic operations (`+`, `-`, `.clamp()`). Replace raw addition/subtraction with `.saturating_add()`, `.saturating_sub()`, and explicitly sanitize `max` bounds (e.g. `.clamp(min, max.max(0))`).
