**SmallVec in BlockMap Intersections**
**Learning:** We replaced Vec with SmallVec in doom-game/src/movement.rs in `check_position_blockmap` where `spechit` and `seen` are allocated frequently, avoiding heap allocations on hot paths.
**Action:** Always consider `SmallVec` when a collection stays small and is created often during high frequency engine operations.

**SmallVec in BlockMap Intersections**
**Learning:** We replaced Vec with SmallVec in doom-game/src/movement.rs in `check_position_blockmap` where `spechit` and `seen` are allocated frequently, avoiding heap allocations on hot paths.
**Action:** Always consider `SmallVec` when a collection stays small and is created often during high frequency engine operations.
