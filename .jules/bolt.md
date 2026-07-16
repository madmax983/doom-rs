**[Replace Vec with SmallVec in move_spechit]
**Learning:** Returning `SmallVec<[usize; 8]>` from `move_spechit` avoids heap allocations since the number of special lines crossed in a single move is usually very small. We must ensure caller types like `actions.rs` match the returned `SmallVec`.
**Action:** Always check if a frequently called hotpath returning a small `Vec` can be replaced with `SmallVec` to eliminate heap allocation overhead.
**[Append instead of overwrite]
**Learning:** Overwriting the journal file with `>` deletes historical learnings which violates the persona constraint. Always use the `>>` append operator to add new entries.
**Action:** Use `cat << 'EOF' >> .jules/bolt.md` for journal entries to preserve history.
