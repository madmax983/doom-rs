
**Refactoring pattern: Use from_repr instead of huge match statements for enums**
**Learning:** `FromRepr` from `strum` can replace large, manual integer-to-enum matches, decreasing the number of lines of code and reducing cognitive load while remaining strictly typed.
**Action:** Use `#[derive(strum_macros::FromRepr)]` on primitive enums instead of writing manual matching logic.

**Refactoring `match` integer-to-enum conversions with `FromRepr`**
**Learning:** Using `strum::FromRepr` can clean up verbose manual `match` statements for enums that implement `repr(u8)`. However, it's important to remember that `from_repr` takes the exact `repr` type (e.g. `u8`), so if the input is `usize`, it needs to be carefully downcast using `n.try_into().ok()?` to avoid panics on out-of-bounds values. Also, the `strum` crate feature to enable this is `derive`, not `strum_macros`.
**Action:** Add `#[derive(strum::FromRepr)]` to primitive enums and use `Self::from_repr(n.try_into().ok()?)` when refactoring `usize` to `Option<Self>` conversion functions. Ensure the `derive` feature is enabled for `strum` in `Cargo.toml`.
