1. **OOM in `doom_net::SnapshotRing`:**
   - I have written a fuzzer that found an OOM because `capacity` from `fuzz_target_1` is taken directly from the arbitrary data. Wait, actually, `SnapshotRing`'s capacity comes from arbitrary input, but in actual code, is it bounded? Let's check `RollbackManager::new()` passes `MAX_ROLLBACK_TICS` which is 8. But `SnapshotRing::new(capacity)` allows any capacity. It should probably be capped to `1024` or similar, or just return an error/panic? Wait! I found OOMs in `SnapshotRing::new` and `InputLog::new`. But wait, in `InputLog::new`, the capacity is also unrestricted. Wait, Havoc boundaries say "If I can crash it, I win". So I will write tests showing I can crash it, and present the wreckage. But I also need to *fix* the vulnerabilities. Wait, Havoc's job is: "Red Phase: Write tests that fail. Green Phase: Write the minimal amount of code to make those tests pass. Refactor."

2. **Vulnerability in `savegame.rs` (`load_game`):**
   - The fuzzer found an OOM in `doom_game::savegame`! The payload `[255, 255, 255, 255]` causes an allocation of 4GB via `Vec::with_capacity`.
   - Ah! Let's check `load_game_doomrs`.
   - `WriteCursor::new` could OOM, but the read logic where we deserialize does something like `let count = r.read_u32()?; let mut vec = Vec::with_capacity(count as usize);`. Let's check `savegame.rs` line `1060` (or similar) - where we do `Vec::with_capacity(door_count)`. If `door_count` is large, we OOM! The fuzzer actually hit line 118: `WriteCursor::new(capacity)`. Wait, no, the OOM happened on `let mut active_doors = Vec::with_capacity(door_count);`. The code *does* check `if door_count > max_doors { return Err(...) }`, but what if `door_count` is large, and `max_doors` is also large?
   Wait, if `r.data.len()` is 4 bytes, `max_doors` is 0. So it should return `Err`. Wait! The fuzzer failed with `[255, 255, 255, 255]`. Wait, 255 255 255 255 is just 4 bytes!
   Ah, `let max_doors = (r.data.len().saturating_sub(r.pos)) / 36;` If `r.data.len()` is small, `max_doors` is 0. If `door_count` is `0xFFFFFFFF` which is > 0, it should return `Err(SaveError::Truncated)`.
   Let's see where the fuzzer actually OOM'd:
   `#7 0x564c626da174 in doom_game::savegame::WriteCursor::new`
   Wait! The stack trace says `WriteCursor::new`!
   Who calls `WriteCursor::new`? `save_game_doomrs` does `let mut w = WriteCursor::new(4096);`.
   Wait, does `load_game` call `WriteCursor::new`? Let's check.
