with open("crates/doom-game/src/state.rs", "r") as f:
    content = f.read()

# Add `use crate::movers::*;` back, because we need DoomRng and SectorMovers.
# Wait, DoomRng wasn't in movers, it was right before SectorMovers, so it got extracted to movers.rs! Let's check movers.rs.
