Plan:
1. Examine `crates/doom-renderer/src/render.rs` lines 1282: `let cache = flat_cache.unwrap();`.
`doom-renderer/src/render.rs:1282` expects `flat_cache` to be `Some(...)`. If `flat_cache` is `None` but we are rendering `visplanes`, it will panic! Oh wait! Let's check `doom-renderer/src/render.rs` line 1270. It checks `if flat_cache.is_none() { return; }` before unwrapping!

Wait! What about `doom-renderer/src/menu_render.rs:364`?
`unwrap_or(layout.item_ys.first().copied().unwrap_or(60))`
If the menu is empty, `first().copied()` will be `None`, and it defaults to 60.

What about `doom-game/src/savegame.rs`? `load_game(&save_game(gs, ...)).unwrap().state` - these are in tests!

Wait, `doom-game/src/dehacked.rs`:
`let num_str = thing_part.split_whitespace().next().unwrap_or("");`
This is completely safe.

Wait! I missed something! `crates/doom-map/src/lumps.rs` has `try_into().unwrap()`:

```rust
impl Sidedef {
    const BYTE_SIZE: usize = 30;

    fn from_bytes(b: &[u8]) -> Self {
        Self {
            x_offset: i16::from_le_bytes([b[0], b[1]]),
            y_offset: i16::from_le_bytes([b[2], b[3]]),
            upper_texture: b[4..12].try_into().unwrap(),
            lower_texture: b[12..20].try_into().unwrap(),
            middle_texture: b[20..28].try_into().unwrap(),
            sector: u16::from_le_bytes([b[28], b[29]]),
        }
    }
```

Wait, `b[4..12]` is exactly 8 bytes! So `try_into()` an array of 8 bytes `[u8; 8]` is mathematically guaranteed to work!
But can I replace this `.unwrap()` with `[b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11]]` or use a custom test to ensure Sentry is happy? Actually, the user specifically mentioned `.unwrap() on dynamic data`. Wait, Sentry philosophy: `unwrap() is a ticking time bomb—test the explosion or remove the fuse`. I should just remove the fuse by using `.try_into().unwrap_or_default()` or explicit byte array. But `unwrap_or_default` hides bugs.

Let's look at `doom-game/src/trace.rs`:
```rust
    #[test]
    fn ray_hits_perpendicular_wall_at_midpoint() {
        let t = ray_linedef_intersection(0.0, 0.0, 1.0, 0.0, 100.0, -50.0, 100.0, 50.0);
        assert!(t.is_some(), "should hit perpendicular wall");
        let t = t.unwrap();
```
Tests!

What about `doom-app/src/main.rs:4116`?
```rust
        let args = args.unwrap();
```
Tests!

Ah! Wait! Look at `doom-game/src/linedef_dispatch.rs`:
```rust
        let trigger = classify_trigger(special).unwrap();
```
This is NOT a test! It's in `dispatch_walk_lines` function!
```rust
fn dispatch_walk_lines(...) {
    // ...
    let mut walk_lines: Vec<(i64, i64, usize, u16)> = level
        .linedefs
        .iter()
        .enumerate()
        .filter_map(|(i, ld)| {
            if ld.special == 0 {
                return None;
            }
            match classify_trigger(ld.special) {
                Some(TriggerType::WalkOnce) | Some(TriggerType::WalkRepeat) => {
                    // ...
                    Some((frac, dist, i, ld.special))
                }
                _ => None,
            }
        })
        .collect();

    // ...

    for (_, _, ld_idx, special) in walk_lines.into_iter().rev() {
        let trigger = classify_trigger(special).unwrap();
        dispatch_linedef(gs, level, ld_idx, special, trigger, actor, 0);
    }
}
```
If we change `walk_lines` to `Vec<(i64, i64, usize, u16, TriggerType)>`, we can **remove the fuse** entirely!

Let's do that!
