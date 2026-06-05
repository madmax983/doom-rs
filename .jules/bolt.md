**[Eliminate Vector Allocation in sectors_by_tag]**
**Learning:** `sectors_by_tag` collected indices into a `Vec<usize>` resulting in a heap allocation. By returning `impl Iterator<Item = usize> + '_` we prevent vector allocation inside this function which is used across `linedef_dispatch.rs`.
**Action:** Always favor returning iterators in lookup functions to avoid unnecessary heap allocations when the callers only iterate through the result.

**[Eliminate Vector Allocation in sectors_by_tag]**
**Learning:** `sectors_by_tag` collected indices into a `Vec<usize>` resulting in a heap allocation. By returning `impl Iterator<Item = usize> + '_` we prevent vector allocation inside this function which is used across `linedef_dispatch.rs`.
**Action:** Always favor returning iterators in lookup functions to avoid unnecessary heap allocations when the callers only iterate through the result.
