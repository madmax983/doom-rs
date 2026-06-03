⚒️ Forge: Refactor sidedef sector extraction guard clauses

🚮 **Smell:** `activate_doors` repeatedly used a verbose 7-line block to extract a `sector_idx` from `left_sidedef`. This pattern was repeated 16 times inside a `match` statement, creating verbose boilerplate.
✨ **Solution:** Hoisted the exact 7-line boilerplate up before the `match` statement, executing it once, drastically reducing code duplication, vertical space, and cognitive load without changing logic or exceeding line length limitations.
🧼 **Benefit:** Eliminates 112 redundant lines and conforms to DRY principles, ensuring correct formatting and readability.
🛡️ **Verification:** Tests passed. No logic changed.
