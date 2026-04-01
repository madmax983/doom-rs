
## 2025-05-24 - OOM on Blockmap Parsing

**🧨 The Trigger:**
A fuzzed input to `Blockmap::parse_lump` containing a large `x_count` and `y_count` resulted in a massive initial allocation of `Vec::with_capacity(n_blocks)`. Specifically, `n_blocks` reached ~4.2 billion due to the `u16::MAX * u16::MAX` limits, leading to an attempt to allocate over 2.5GB of memory.

**📉 The Stack Trace:**
`libFuzzer: out-of-memory (malloc(2516582400))`

**🧪 Reproduction:**
Run `cargo fuzz run fuzz_target_9` with a malformed BLOCKMAP lump. For instance, `fuzz/artifacts/fuzz_target_9/oom-b45e5be191f1be957f38ac76c7d1c30af5cdda79`.

**😈 Comment:**
You blindly trusted the dimensions in the file header to allocate an unbounded `Vec`. Don't trust fuzzed integer fields for initial allocations. Always bound them by `data.len()`.
