**SmallVec for Walk Lines Allocation**
**Learning:** `Vec::new()` is heavily used during collision detection on the hot loop (e.g. `walk_lines.sort_by`). Replacing this with `smallvec::SmallVec` stops dynamic allocations for small intersection arrays.
**Action:** Use `smallvec::SmallVec<[T; N]>` where small static allocations cover 99% of cases on performance-critical paths.
