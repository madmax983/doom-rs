**[Optimized Collects in `doom-app/src/main.rs`]**
**Learning:** `.collect::<Vec<_>>()` chains in hot paths can cause unnecessary heap allocations. Using `.into_iter()` or `.iter()` directly or using `Vec::with_capacity()` reduces heap allocations. Replaced a collect chain in `main.rs` with `Vec::with_capacity(256)` and `actors.extend(...)`. Also replaced `Vec::new()` usages for building WAD lumps with `.with_capacity()`.
**Action:** Always check for `Vec::new()` or `.collect()` in rendering loops or data generation methods and replace them with iterators or `.with_capacity()` when the bounds are known.

**[Optimized Collects in `doom-app/src/main.rs`]**
**Learning:** `.collect::<Vec<_>>()` chains in hot paths can cause unnecessary heap allocations. Using `.into_iter()` or `.iter()` directly or using `Vec::with_capacity()` reduces heap allocations. Replaced a collect chain in `main.rs` with `Vec::with_capacity(256)` and `actors.extend(...)`. Also replaced `Vec::new()` usages for building string splits with iterator matching `(Some(p1), Some(p2), None)`.
**Action:** Always check for `Vec::new()` or `.collect()` in rendering loops or data generation methods and replace them with iterators or `.with_capacity()` when the bounds are known.
