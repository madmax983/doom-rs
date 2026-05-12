**Consolidating match blocks**
**Learning:** Large match blocks with duplicate arm bodies can be cleanly refactored into multi-pattern arms (`A | B => { ... }`) to significantly reduce lines of code and improve readability.
**Action:** When finding massive repeating `match` arms, programmatically extract and group them, then carefully replace them while maintaining original execution blocks.
