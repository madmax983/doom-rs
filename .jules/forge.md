**Refactor operator precedence clarity**
**Learning:** Shift operators combined with multiplication/division without parentheses (e.g. `16 * 64 << 16`) can trigger clippy precedence warnings and reduce readability because the evaluation order is not immediately obvious to readers.
**Action:** Always wrap the inner mathematical operations in parentheses (e.g., `(16 * 64) << 16`) before bitwise shifting to explicitly state the intended evaluation order and satisfy `clippy::precedence`.
