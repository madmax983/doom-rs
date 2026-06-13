sed -i 's/pub use doom_types::limits::{NUM_POWERS, NUM_PSPRITES};/pub(crate) use doom_types::limits::{NUM_POWERS, NUM_PSPRITES};/g' crates/doom-game/src/player.rs
sed -i 's/f.buffer_mut().cell_mut((x, y)).map(|c| c.set_skip(true));/#[allow(deprecated)]\n                                f.buffer_mut().cell_mut((x, y)).map(|c| c.set_skip(true));/g' crates/doom-tui/src/event_loop.rs
sed -i 's/buf.cell_mut((x, y)).map(|cell| cell.set_skip(true));/#[allow(deprecated)]\n                buf.cell_mut((x, y)).map(|cell| cell.set_skip(true));/g' crates/doom-tui/src/sixel.rs
