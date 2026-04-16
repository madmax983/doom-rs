1. Refactor `switch.rs` logic to remove nesting and flatten variables via guard clauses using `if let` to `let else`. *(Completed)*
2. Refactor `dehacked.rs` by removing nested `if let` checks and flattening the code via `let else`. *(Completed)*
3. Refactor `pickups.rs` logic to remove `match` statement for single checks and flatten it to `if / else if`. *(Completed)*
4. Refactor `sound.rs` to flatten nested `match`es with `let else`. *(Completed)*
5. Refactor `spawn.rs` to flatten nested `match` with `let else` and use tuple destruction to simplify code. *(Completed)*
6. Refactor `sight.rs` to use `let else` and simplify nested `match` statements. *(Completed)*
7. Refactor `projectile.rs` to use `let else` instead of `match` for fetching handle values. *(Completed)*
8. Refactor `phase.rs` to replace `match` with `if let` when other arms are empty. *(Completed)*
9. Update `.jules/forge.md` with new findings.
10. Run pre-commit steps.
11. Submit changes as a PR with `submit`.
