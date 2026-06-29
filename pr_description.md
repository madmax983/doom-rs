🧨 **The Trigger:**
* Division by zero in `Fixed16_16::fixed_div`: Zero checks only used `debug_assert!`, allowing release builds to either divide by zero or require complex control flows to handle it if optimizations misbehave.
* Untrusted `u32` string length allocation in `savegame.rs`: An attacker can specify a massive name length causing `checked_add` and out-of-bounds validations to fail, but it's better to explicitly clamp memory-exhausting bounds.

📉 **The Stack Trace:**
* `fixed::tests::fixed_div_by_zero_panics` failed because the `debug_assert!` was removed or suppressed, leading to actual divide by zero logic triggering instead of safely handling the bounds.

🧪 **Reproduction:**
* `cargo test -p doom-types`
* `cargo fuzz run fuzz_target_savegame_doomrs`

😈 **Comment:**
"You assumed the buffer would never be larger than RAM, and that users wouldn't deliberately feed you zeros. You were wrong."
