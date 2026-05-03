**[Optimized sector tag lookup]
**Learning:** Returning a `Vec` for small, transient filter operations introduces unnecessary heap allocation. By returning `impl Iterator<Item = usize> + '_` instead, we keep the iteration entirely zero-cost and avoid allocating and filling intermediate vectors.
**Action:** Replace `Vec` creation in simple tag/filter lookup functions with `impl Iterator` to reduce allocator pressure and improve performance.
**[Optimized sector tag lookup]
**Learning:** Returning a `Vec` for small, transient filter operations introduces unnecessary heap allocation. By returning `impl Iterator<Item = usize> + '_` instead, we keep the iteration entirely zero-cost and avoid allocating and filling intermediate vectors.
**Action:** Replace `Vec` creation in simple tag/filter lookup functions with `impl Iterator` to reduce allocator pressure and improve performance.
