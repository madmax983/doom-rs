## 2024-05-18 - [Sentry: Expect over Unwrap]
**Learning:** `unwrap()` is a code smell that can lead to untracked panics. When refactoring or encountering `.unwrap()`, it's better to replace it with `.expect()` and a clear message explaining why the unwrap is safe, providing context for the invariant.
**Action:** Always prefer `.expect()` over `.unwrap()` in production code to document assumptions and aid debugging.
