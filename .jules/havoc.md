## 2024-05-18 - [Havoc: OOM on TEXTURE1 parser]
**Learning:** Doom's TEXTURE1 parsing uses a direct 4-byte `num_textures` read to allocate `Vec::with_capacity(num_textures)`. Fuzzing this length with large values triggers an immediate OOM.
**Action:** Use `.min(data.len() / 4)` to clamp lengths derived from WAD/lump headers, preventing massive allocations while still ensuring we parse valid entries up to the slice boundary.
**[Integer Overflows in State Mutations]**
**Learning:** `PlayerState` health and armor mutations incorrectly assumed safe, bounded input values, leading to `min > max` panics in `clamp()` when providing massive negative caps, or `attempt to subtract with overflow` panics when deducting `i32::MIN` armor. Fuzzed inputs (like broken Save Games or DehPatch mods) easily bypassed "Happy Path" design limits.
**Action:** When implementing any mathematical bounds on fuzzed or state-dependent inputs, ALWAYS use `.saturating_add()` and `.saturating_sub()`. Before invoking `.clamp(min, max)` or `.min(max)`, explicitly sanitize the upper bound to ensure `min <= max` (e.g., `let safe_cap = cap.max(0)`).
