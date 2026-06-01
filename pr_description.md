🎯 Target: Added test coverage to `sprite_clip.rs` and `wad_font.rs` in `doom-renderer`.
💣 Risk: Previously, critical font glyph spacing and sprite clipping behaviors were completely untested, increasing the risk of subtle rendering regressions or off-by-one errors when clipping arrays overflowed.
🧪 Strategy: Added robust unit tests verifying that `SpriteClipHistory` correctly tracks state and safely drops extra clips when reaching its capacity limit of 8. For `WadFont`, added tests that mock `PatchImage` to verify accurate string width calculations, ensuring correct gap logic and missing-glyph fallbacks.
🔬 Verification: Run `cargo test --package doom-renderer --lib sprite_clip` and `cargo test --package doom-renderer --lib wad_font`.
