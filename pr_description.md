🕸️ Tangle: The `ExitRequest` and `LockedDoorColor` enums were defined in `doom-game/src/state.rs` but widely used in presentation crates like `doom-app` for rendering UI messages and orchestrating game loops. This caused UI and outer orchestrator components to depend on the core game engine logic just for basic enum definitions, coupling presentation with the simulation loop.

📐 Blueprint: Extracted `ExitRequest` and `LockedDoorColor` into `doom-types/src/game_enums.rs` (a new file). Updated `doom-game/src/state.rs` and `doom-game/src/lib.rs` to re-export them for internal backward compatibility, and updated `doom-app` imports to reference the primitive shared types directly.

🧱 Stability: Reduced coupling between the orchestration layer (`doom-app`) and the simulation layer (`doom-game`). Presentation now depends on foundational types instead of core game logic, enforcing cleaner domain boundaries and slightly faster compile times.

🔬 Verification: Code compiles successfully. All existing tests pass. Verified the imports in `doom-app` accurately point to `doom-types`.
