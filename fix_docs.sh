sed -i 's/(\[`crate::movement::record_player_crossings`\])/(`crate::movement::record_player_crossings`)/g' crates/doom-game/src/linedef_dispatch.rs
sed -i 's/(\[`record_player_crossings`\])/(via `record_player_crossings`)/g' crates/doom-game/src/movement.rs
sed -i 's/\[`point_on_side_fixed`\]/`point_on_side_fixed`/g' crates/doom-map/src/bsp.rs
sed -i 's/\[`TIC_DURATION`\](doom_tui::TIC_DURATION)/\[`TIC_DURATION`\]/g' crates/doom-present/src/tics.rs
