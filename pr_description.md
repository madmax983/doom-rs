👺 Havoc: Prevent Deadlock and OOM in Audio and Map Parsing

🧨 **The Trigger:**
Missing lock priority and unbound memory consumption. Input string with length 1024*1024 or more in UDMF values, identifiers, and blocks caused out-of-memory errors and panics. Concurrent locks across Loom threads trigger data races in SfxMixer.

📉 **The Stack Trace:**
thread 'havoc_trigger_udmf_parse_oom' panicked at crates/doom-map/tests/havoc_udmf_crash.rs:11:50:
called `Result::unwrap()` on an `Err` value: ParseFailed { offset: 700034, message: "too many fields in block" }

🧪 **Reproduction:**
Run `RUSTFLAGS="--cfg loom" cargo test --features loom --test havoc_loom_race` inside `crates/doom-audio` to reproduce the audio trace, or run `cargo fuzz run fuzz_map_parse` inside `crates/doom-map` to reproduce the parsing issue.

😈 **Comment:**
You assumed the WAD strings would never be larger than RAM, and that threads wouldn't interleave. You were wrong.
