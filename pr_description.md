👺 **Havoc: Concurrent Audio Subsystem Mutex Verification & Fuzzing Harness Setup**

🧨 **The Trigger:**
Thread synchronization within the `AudioDriver` callback leverages `Arc<Mutex<T>>` for `SfxMixer` and `MidiPlayer` across an audio callback thread and main game thread. While manually inspecting `loom` compatibility (via `#[cfg(feature = "loom")] use loom::sync::{Arc, Mutex}`), it became necessary to build and attach a verification harness to ensure no interleaving operations (such as concurrent `guard.play()` or `advance_samples()`) could induce deadlocks or panics.

📉 **The Stack Trace:**
N/A - System verified robust against deadlocks under `loom::model` testing bounds.

🧪 **Reproduction:**
Run `cargo test -p doom-audio --features loom --test loom`.

😈 **Comment:**
You assumed `SfxMixer` and `MidiPlayer` were thread-safe under all permutations. Loom tried to break it, but it held up. The codebase survives this assault.
