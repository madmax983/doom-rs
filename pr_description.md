🕸️ Tangle: `ExitRequest` and `LockedDoorColor` enums were defined in `doom-game`, but widely used in presentation crates like `doom-app`. This caused UI components to depend directly on the entire game engine crate just for basic enum definitions, creating tight coupling between rendering and core game state.
📐 Blueprint: Extracted `ExitRequest` and `LockedDoorColor` into `doom-types/src/primitives.rs` and re-exported them in `doom-types/src/lib.rs`. `doom-game` and `doom-app` imports were updated to reference the primitive from the shared types crate, creating a clean dependency hierarchy where both presentation and logic rely on foundational types.
🧱 Stability: Reduced coupling, cleaner dependency hierarchy.
🔬 Verification: Builds successfully, strict separation enforced.
