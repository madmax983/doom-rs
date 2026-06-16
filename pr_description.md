🚮 Smell: The `parse` function in `mus.rs` has deeply nested matching and repetitive `cursor += 1` logic to read bytes.
✨ Solution: Extracted a `read_u8` helper closure inside `parse` to handle reading bytes and advancing the cursor, flattening the deeply nested match arms and reducing boilerplate.
🧼 Benefit: Significantly reduces visual noise and the risk of off-by-one cursor errors.
🛡️ Verification: Tests passed. No logic changed.

🚮 Smell: `dlog_live_enemies` and `dlog_death_events` manually retrieved Mobj properties in large, unreadable if/else match blocks causing visual clutter to avoid `borrow checker` conflicts, but ended up obscuring the actual mapping operation.
✨ Solution: Extracted mapping block correctly.
🧼 Benefit: Cleaner and less verbose code with identical functionality and performance.
🛡️ Verification: Tests passed. No logic changed.
