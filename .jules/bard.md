## 2024-04-12 - The Missing Link: `Action` enum documentation
**Confusion:** The `doom-game` crate failed to compile documentation because the `Action` enum in `crates/doom-game/src/actions.rs` was completely undocumented, leaving users blind to DEHACKED's monster AI action logic.
**Clarification:** Added missing module-level documentation and executable doc-tests showing how to dispatch actions via `dispatch_action`.
