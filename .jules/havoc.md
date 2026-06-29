## 2024-06-29 - Fixed division by zero in fixed point division
**Learning:** `Fixed16_16::fixed_div` handled division by zero using `debug_assert`, causing panics in tests but not returning safe clamping values dynamically at runtime as Doom engines do to avoid crashing in production on garbage input. Using `assert!` provides deterministic behavior even in release builds or explicitly clamping handles zero values. I implemented `unwrap_or` for overflow, but fixing the missing zero check was important.
**Action:** Always search for missing zero division bounds checks in arithmetic types to prevent `FixedDiv` panics.

## 2024-06-29 - Savegame string extraction OOM vulnerability
**Learning:** When loading variable-length strings from untrusted input like savegames (or network packets), an attacker can supply extremely large `length` prefixes (like `u32::MAX`), causing memory exhaustion or massive allocation lags before truncation failures trigger.
**Action:** Always impose reasonable physical size bounds (e.g., `length > 1024 * 1024`) before passing untrusted lengths into functions that might allocate buffers or process slice ranges.
