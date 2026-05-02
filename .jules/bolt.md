**[Return lazy Iterators with references]**
**Learning:** Returning a lazy `impl Iterator` to avoid `Vec` allocations extends the immutable borrow of the underlying collection. If the calling scope requires mutable access to related state during iteration, this causes borrow checker conflicts (`E0502: cannot borrow as mutable because it is also borrowed as immutable`).
**Action:** To resolve this, eagerly collect the results into a `smallvec::SmallVec` to drop the borrow immediately while keeping small lists on the stack.
