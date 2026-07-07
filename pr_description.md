🧨 **The Trigger:** Fuzzing blockmap lumps exposed that querying out-of-bounds `(x,y)` block coordinates caused the code to fallback to offset `0`, treating the map header as a linedef index array and returning garbage values instead of an empty result.
📉 **The Stack Trace:** (Assertion failure in test: `left == right` failed: out of bounds block should be empty, but yielded: `Some(0)`).
🧪 **Reproduction:** Run `cargo test -p doom-map --test test_havoc`.
😈 **Comment:** You assumed the user wouldn't look for things outside the bounds of reality. You were wrong.
