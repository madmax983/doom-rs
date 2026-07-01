🗺️ Atlas: [Abstract SaveError via NewType]
🕸️ Tangle: The `SaveError` in `doom-app` was redefining engine-level variants (`BadMagic`, `Truncated`) and mapping them manually via a `From` block, which leads to maintenance overhead.
📐 Blueprint: Wrapped `doom_game::savegame::SaveError` inside a new `Engine` variant on `doom_app::savegame::SaveError` and removed the manual `From` implementation, allowing `thiserror` to handle it.
🧱 Stability: Reduced code duplication and tightened the error boundary between application and engine.
🔬 Verification: Builds successfully, all tests pass.
