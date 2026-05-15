**Overflow in Combat radius_attack**
**Learning:** Found an integer overflow where multiplying maximum `i32` damage by `(radius - dist)` would easily panic standard `i32` bounds if the values were maliciously large (like from a fuzz target).
**Action:** Always cast bounds and modifiers to `i64` before multiplication during distance scaling, then clamp to `i32::MIN..i32::MAX` before casting back.
