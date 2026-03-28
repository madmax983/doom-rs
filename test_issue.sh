#!/bin/bash
# Re-export of doom_game::AutomapState from doom-renderer creates a leaky abstraction.
# We should import it directly from doom_game in doom-app.

# Remove the pub use from doom-renderer
sed -i 's/pub use doom_game::AutomapState;//' crates/doom-renderer/src/lib.rs

# In doom-app/src/main.rs, remove AutomapState from doom_renderer imports
# and add it to doom_game imports.
sed -i 's/    ActorRenderInfo, AnimState, AutomapState, BitmapFont/    ActorRenderInfo, AnimState, BitmapFont/' crates/doom-app/src/main.rs
sed -i 's/use doom_game::{/use doom_game::{AutomapState,\n/' crates/doom-app/src/main.rs

cargo check
