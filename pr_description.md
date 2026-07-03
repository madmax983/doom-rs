🚮 Smell: The `tick()` function in `crates/doom-app/src/main.rs` is almost 300 lines long and acts as a "God Function", handling menu ticking, title screens, intermission, in-game menu, console text input, cheats, quick save/load, game simulation, sound event dispatch, player damage palette flashes, and debug logging. It is hard to read and navigate.

✨ Solution: Extracted logically distinct sections of `tick()` into smaller, private helper functions (`tick_title_screen`, `tick_intermission`, `tick_in_game_menu`, `tick_console_and_cheats`, `tick_quick_save_load`, `tick_sound_events`, and `tick_player_damage_and_logging`). Then, updated the `tick` function body to use these new helpers, significantly reducing its length. All inline comments and documentation strings were intentionally preserved.

🧼 Benefit: Improves readability by flattening the structure and reducing cognitive load. The main `tick` loop now clearly reads as a high-level orchestration of different systems. Preserving inline comments ensures valuable context is not lost.

🛡️ Verification: Ran `cargo clippy`, `cargo test`, and `cargo fmt`. No logic changed.
