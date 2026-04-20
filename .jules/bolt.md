# Bolt's Performance Journal

**Replacing `.collect::<Vec<_>>().join()`**
**Learning:** In string generation (like formatting JSON arrays), doing an intermediate `.collect::<Vec<_>>()` before calling `.join(", ")` is wasteful and creates unnecessary heap allocations for each item and the vector itself.
**Action:** Use a pre-allocated `String` and manually loop over the items with an `.enumerate()`, pushing elements and commas sequentially to the buffer to avoid intermediate allocations.
