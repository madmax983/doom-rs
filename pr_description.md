🧨 **The Trigger:** Fuzzing the `SfxMixer` and `AudioDriver` with f32 NaN values through `volume` and `pan` parameters causes `.clamp(-1.0, 1.0)` to panic due to floating point `NaN` poisoning.
📉 **The Stack Trace:**
```rust
thread '<unnamed>' (18852) panicked at fuzz_targets/fuzz_target_3.rs:35:13:
NaN detected!
```
🧪 **Reproduction:** Run `cargo +nightly fuzz run fuzz_target_4` and `cargo +nightly fuzz run fuzz_target_3`.
😈 **Comment:** Floating-point numbers are tricky. NaN is not a number, and clamping it makes no sense. You should have checked `is_nan()`!
