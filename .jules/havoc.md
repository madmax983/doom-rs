**Audio Float NaN Poisoning**
**Learning:** `f32::clamp` throws a panic (`NaN` is passed to it). In Rust, floating point values fetched directly from binary data (which we do if they're random or untrusted inputs) can be `NaN`. When handling arbitrary floating-point inputs, explicit checks for `.is_nan()` are necessary before passing to operations that require total order, like `clamp`.
**Action:** Always check `.is_nan()` when converting bytes to floats or when doing math that can result in `NaN`s, before calling `.clamp()`.
