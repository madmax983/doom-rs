🧨 **The Trigger:** `f32::clamp` throws a panic (`NaN` is passed to it). In Rust, floating point values fetched directly from binary data (which we do if they're random or untrusted inputs) can be `NaN`. When handling arbitrary floating-point inputs (like adding arbitrary buffers in audio callbacks), explicit checks for `.is_nan()` are necessary before passing to operations that require total order, like `clamp`.
📉 **The Stack Trace:**
```
thread '<unnamed>' (37667) panicked at fuzz_targets/fuzz_audio_clamp.rs:19:5:
assertion failed: !result.is_nan()
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
==37667== ERROR: libFuzzer: deadly signal
```
🧪 **Reproduction:** Write a fuzzer script that extracts four bytes from its data feed to form an `f32` and passes it to `.clamp()`. (Can see crash locally by running `cargo +nightly fuzz run <target>`).
😈 **Comment:** "You assumed floats from an audio buffer are never `NaN`. You were wrong. `f32::clamp` panics on `NaN`!"
