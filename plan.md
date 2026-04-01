1. **Analyze performance opportunity**:
   - `render_level_with_view_height_and_extra_light` in `crates/doom-renderer/src/render.rs` allocates `wall_clip_top_history` and `wall_clip_bot_history` per frame using `std::iter::repeat_with(Vec::new).take(SCREEN_W).collect()`.
   - `SCREEN_W` is 320. This causes 640 small `Vec` allocations (and their eventual drops) per frame.
   - For 35 FPS (Doom tick rate), that's `640 * 35 = 22,400` heap allocations per second for sprite clipping history alone.

2. **Optimization strategy**:
   - Change `clip_top_history` and `clip_bot_history` to `[Vec<SpriteClipStep>; SCREEN_W]`.
   - The array can be initialized with `std::array::from_fn(|_| Vec::new())`. Wait, `std::array::from_fn` still allocates `Vec::new()` per frame if done naively.
   - We need to avoid allocations on the hot path. To reuse allocations, `clip_top_history` and `clip_bot_history` should be persisted across frames. However, `render_level` creates a new `RenderOut` every frame.
   - Wait, `RenderOut` is constructed fresh every frame and holds these `Vec<Vec<...>>`. The history arrays are used by `render_actors_with_masked_ex`.
   - If we just change the array initialization to `const { Vec::new() }`, it's still an allocation when elements are added. `Vec::new()` doesn't allocate until elements are pushed. But many columns might get elements pushed, so it still allocates frequently.

Let's look at `masked_columns` and `visplanes`. The `VisplaneSet` reuses allocations internally? No, `VisplaneSet::new()` is created locally in `render_level`.

Wait, we are Bolt ⚡, we want to eliminate intermediate allocations.
What if we replace `wall_clip_top_history: Vec<Vec<SpriteClipStep>>` with `[Vec<SpriteClipStep>; SCREEN_W]`? It avoids the outer `Vec` allocation but the inner `Vec`s still exist. The outer `Vec` allocation is 1 allocation of 320 elements. 2 allocations per frame. That's `2 * 35 = 70` allocs/sec.
No, `std::iter::repeat_with(Vec::new).take(SCREEN_W).collect()` collects into a `Vec<Vec<...>>`. That's 1 outer allocation, but it also creates 320 inner `Vec`s. `Vec::new()` doesn't allocate, but when we push it allocates.

Actually, the exact same pattern is in `crates/doom-renderer/src/sprite.rs` inside `render_actors_ex`:
```rust
        let mut bottom_history: Vec<Vec<crate::render::SpriteClipStep>> =
            std::iter::repeat_with(Vec::new).take(SCREEN_W).collect();
```
In `sprite.rs` there are 3 instances of `std::iter::repeat_with(Vec::new).take(SCREEN_W).collect();`.

Let's change `Vec<Vec<...>>` to an array `[Vec<...>; SCREEN_W]` where we use `std::array::from_fn` or `[const { Vec::new() }; SCREEN_W]`.
Wait! `[const { Vec::new() }; SCREEN_W]` is valid Rust 1.79+! It creates an array of 320 empty Vecs. It does not allocate the outer array dynamically on the heap! The array lives on the stack. `[Vec<SpriteClipStep>; 320]` is `320 * 24 = 7680` bytes, which is perfectly fine for the stack.

Let's check the size of `RenderOut`:
`z_buf`: 320 * 4 = 1280
`clip_top`: 320 * 4 = 1280
`clip_bot`: 320 * 4 = 1280
`clip_top_depth`: 320 * 4 = 1280
`clip_bot_depth`: 320 * 4 = 1280
`clip_top_history`: 320 * 24 = 7680
`clip_bot_history`: 320 * 24 = 7680
`masked_columns`: 24
Total: ~22 KB. This is fine for stack return.

Wait, `SpriteClipStep` size:
`f32` (4), `i32` (4), `f32` (4) = 12 bytes.
`Vec<T>` size = 24 bytes.
320 * 24 = 7680 bytes.

If we change `Vec<Vec<SpriteClipStep>>` to `[Vec<SpriteClipStep>; SCREEN_W]`, we remove 2 heap allocations per frame in `render_level` and up to 3 per frame in `sprite.rs`.
Wait, there is an even better way to avoid allocations for the inner `Vec`s: SmallVec! But we are told "Adding any new crates to Cargo.toml" is ⚠️ Ask first. We can just use `[Vec; SCREEN_W]` which removes the outer allocation.

Is there any other low-hanging fruit with `collect::<Vec<_>>`?
