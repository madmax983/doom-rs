🧨 **The Trigger:** `Fixed16_16::fixed_div` was silently swallowing division by zero and returning `i32::MAX` or `i32::MIN` instead of failing fast. The proptests and test expectations were also incorrectly constructed to enforce this unsafe silence.
📉 **The Stack Trace:** No stack trace, because it didn't panic! It silently corrupted data. Now it properly panics: `panic!("FixedDiv: division by zero");`.
🧪 **Reproduction:** Run `cargo test -p doom-types` (Specifically `fixed_div_by_zero_panics` which is now passing as it actually panics).
😈 **Comment:** "You assumed division by zero was an edge case you could sweep under the rug. I proved you wrong. It's a fatal error. Face reality."
