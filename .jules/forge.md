# Forge Journal

**[Refactoring God Functions in Renderer]**
**Learning:** The `render.rs` module has huge functions (`render_level`, `render_level_with_view_height`) passing 11-13 arguments. This needs to be packaged into structs.
**Action:** Create a config or context struct for `render_level` variants to reduce parameter counts.
