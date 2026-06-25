🗺️ Atlas: Extract SectorDamageType and LightEffectType to Shared Primitives

🕸️ Tangle: The `SectorDamageType` and `LightEffectType` primitive enums were defined in `doom-game`, causing game engine components to depend directly on the entire game engine crate just for basic limits and bounds representing map features.
📐 Blueprint: Extracted `SectorDamageType` and `LightEffectType` into `doom-types/src/primitives.rs`. `doom-game` components that relied on them were updated to import these primitives directly from the shared types crate.
🧱 Stability: This creates a clean dependency hierarchy where presentation and engine logic rely on foundational limits instead of pulling them through the game logic, reducing coupling.
🔬 Verification: Builds successfully, strict separation enforced. Tests and clippy pass cleanly.
