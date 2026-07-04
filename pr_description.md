🚮 Smell: Boolean Blindness in `hitscan_shot_angle` where passing `true` obscures the intent of the accuracy parameter at the call site.
✨ Solution: Extracted an explicit enum `HitscanAccuracy` (`AccurateFirstShot` and `Spread`) to strongly type the accuracy state and replace the boolean.
🧼 Benefit: Improves call site readability and prevents semantic ambiguity by replacing raw boolean values with descriptive type variants.
🛡️ Verification: Tests passed. No logic changed.
