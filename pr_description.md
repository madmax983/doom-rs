👹 Havoc: Fix integer overflow and OOM vectors in Map Lumps

🧨 **The Trigger:**
Providing extreme values for `col` and `row` when querying blockmap indices, or manipulating the header's `x_count` and `y_count` causes immediate integer overflow leading to panics.

📉 **The Stack Trace:**
```
thread '<unnamed>' panicked at /app/crates/doom-map/src/lumps.rs:690:19:
attempt to multiply with overflow
```

🧪 **Reproduction:**
Run `cargo fuzz run fuzz_target_7`

😈 **Comment:**
"You assumed we would never query an index that could multiply past usize::MAX. You were wrong."
