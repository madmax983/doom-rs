**[Title: Avoid intermediate `.collect::<Vec<_>>()` before Ratatui widgets]
**Learning:** Passing an iterator directly to widget constructors (like `List::new()`) avoids an intermediate heap allocation because they implement `IntoIterator`.
**Action:** Next time, look for `.collect::<Vec<_>>()` calls whose sole purpose is to feed a constructor that accepts `IntoIterator`.
