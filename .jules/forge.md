
**Refactoring pattern: Use from_repr instead of huge match statements for enums**
**Learning:** `FromRepr` from `strum` can replace large, manual integer-to-enum matches, decreasing the number of lines of code and reducing cognitive load while remaining strictly typed.
**Action:** Use `#[derive(strum_macros::FromRepr)]` on primitive enums instead of writing manual matching logic.

**Refactoring pattern: Use from_repr for primitive enum mappings**
**Learning:** Large manual `match` statements mapping primitive integers to enum variants are verbose and error-prone. By applying `#[derive(strum_macros::FromRepr)]` and `#[repr(u*)]` to the enum, and assigning explicit discriminants to the variants, we can use `Enum::from_repr(value)` to cleanly and safely handle the conversion.
**Action:** When encountering a large `match` block mapping integers to an enum, check if the enum can be modified to use explicit discriminants matching the integers. If so, add `FromRepr` and replace the `match` block with `Enum::from_repr`.
