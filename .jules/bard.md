## 2024-05-21 - [Added Sprite Clip and General Module Docs]
**Confusion:** The purpose and capacity choice of `SpriteClipHistory` was implicit, making it unclear why a bounded array was used instead of a standard `Vec`. Several modules also lacked basic `//!` documentation, appearing as black boxes.
**Clarification:** Added module-level documentation across the workspace to clear lints, and rewrote `SpriteClipHistory` documentation with full executable doctests to explicitly demonstrate its usage and the architectural decision to drop elements over capacity to save heap allocations.
