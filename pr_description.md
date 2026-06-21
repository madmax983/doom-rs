🚮 Smell: `chokepoints` was a God Function with a Pyramid of Doom, maintaining 6 state variables and deep nesting for iterative DFS. `isolated_areas` suffered from nesting instead of guard clauses. Option destructuring caused boolean blindness.
✨ Solution: Extracted the 6 variables into `ChokepointsState`. Extracted the DFS into `find_chokepoints_from_node`. Flattened nesting using guard clauses. Inlined Option destructuring to avoid intermediate variables.
🧼 Benefit: Reduces cognitive load and flattens logic, adhering to idiomatic Rust.
🛡️ Verification: Tests passed. No logic changed.
