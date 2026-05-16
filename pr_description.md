🧨 **The Trigger:** Fuzzing the `encode_pcm16_wav_mono` with massive sample slice lengths triggered a multiply-with-overflow when calculating `data_size`. Subsequent allocation crashed due to OOM.
📉 **The Stack Trace:**
```
thread '<unnamed>' (44311) panicked at /app/crates/doom-audio/src/wav.rs:85:26:
attempt to multiply with overflow
==44311== ERROR: libFuzzer: deadly signal
```
🧪 **Reproduction:** Run `cargo fuzz run wav_encode` from the added fuzz target.
😈 **Comment:** You assumed the output buffer would fit in memory and that `u32` wouldn't overflow. You were wrong.
