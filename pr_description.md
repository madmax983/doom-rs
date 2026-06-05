💡 What:
- Derived `Copy` on `MusEvent`.
- Removed `.clone()` inside the tight audio rendering loop of `MidiPlayer::advance_samples`.

🎯 Why:
- `MusEvent` only contained `u8` and `Option<u8>`. Being implemented as an enum without `Copy` forced the sequencer to clone on every single event lookup, introducing memory overhead during hot path execution.

📊 Impact:
- Zero cost abstractions. Eliminates per-event `clone` overhead during audio playback, preventing tight loop allocations.

🔬 Measurement:
- Run `cargo test` to ensure midi rendering output is fully deterministic.
