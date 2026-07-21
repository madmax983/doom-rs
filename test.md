We need to add module level documentation to:
- `crates/doom-app/src/lib.rs` (empty file right now)
- `crates/doom-game/src/director.rs`
- `crates/doom-game/src/movers.rs`
- `crates/doom-game/src/sound_prop.rs`
- `crates/doom-game/src/stats.rs`
- `crates/doom-renderer/src/sprite_clip.rs`

Also we need to add a missing doc block to `AiDirector::new` and `AiDirector::tick` inside `crates/doom-game/src/director.rs`.
And a missing doc block for methods inside `SpriteClipHistory` in `crates/doom-renderer/src/sprite_clip.rs`.
