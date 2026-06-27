🧊 The Trigger
Fixed-point division via `fixed_div` causes a core panic when dividing `i32::MIN` (`-2147483648`) by `-1`.

📉 The Stack Trace
```
attempt to compute i32::MIN / -1_i32, which would overflow
```

🔬 Reproduction
Run `let _ = Fixed16_16::from_raw(i32::MIN).fixed_div(Fixed16_16::from_raw(-1));` in any codebase.

😈 Comment
You assumed 64-bit integer intermediates would save you from fixed-point overflow. But signed division of `MIN` by `-1` panics in Rust before truncation can occur. Never trust the CPU's arithmetic flags.
