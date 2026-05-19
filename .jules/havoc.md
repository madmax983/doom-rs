**Havoc: Loom Deadlocks with Multiple Mutexes**
**Learning:** When locking multiple Mutexes in different threads, acquiring locks in different orders leads to deadlocks.
**Action:** Always acquire locks in the exact same deterministic order across all threads.
