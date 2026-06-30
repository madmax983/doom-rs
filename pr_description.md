⚒️ Forge: Refactor unwrap_or_default coordinate extraction and boolean blindness

🚮 Smell: Extracting coordinates using `.map(|mo| (mo.x, mo.y)).unwrap_or_default()` in `combat.rs` generated unnecessary intermediate values and suffered from boolean blindness. In `actions.rs`, target checks used `.map(|t| !t.is_dead()).unwrap_or(false)` and `.map(|t| t.is_dead()).unwrap_or(true)`, which was verbose and required evaluating complex booleans.
✨ Solution: Replaced `.map().unwrap_or_default()` with idiomatic `if let Some(mo)` guard clauses in `combat.rs` to extract coordinates directly when needed for telemetry. In `actions.rs`, flattened `.unwrap_or(false)` boolean checks into explicit `if let Some()` and guard clauses.
🧼 Benefit: Removes boolean blindness and the creation of unused zero-default coordinates, significantly improving readability and linearity.
🛡️ Verification: Tests passed. No logic changed.
