🧨 **The Trigger:**
Passing edge-case integer inputs to `segment_intersection_frac` triggers a panic. Specifically, doing `let rdx = i64::from(bx - ax);` where `ax = i32::MIN` and `bx = i32::MAX`. `bx - ax` is computed as an `i32` first, triggering a subtract with overflow panic before the conversion to `i64` happens.

📉 **The Stack Trace:**
```
thread 'specials::havoc_tests::havoc_test_segment_intersection_frac_no_overflow' panicked at crates/doom-game/src/specials.rs:3616:25:
attempt to subtract with overflow
```

🧪 **Reproduction:**
Run `cargo fuzz run fuzz_specials_intersect`.

😈 **Comment:**
You assumed `bx - ax` would automatically elevate to `i64` because the outer function call is `i64::from`. You were wrong.
