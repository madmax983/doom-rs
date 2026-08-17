## 2025-02-23 - Sentry should not test trivial auto-derived traits

**Learning:** Sentry shouldn't write tests for compiler auto-derived traits, like `Default` because it amounts to testing the compiler. Also tests should ideally cover logic, edge cases and panic risks. Simply asserting the distinctness of constants (e.g. testing `a & b == 0`) does not test application logic but rather sanity checks hardcoded constants, falling outside the main "High Risk" directives.

**Action:** Focus on testing actual application behavior, complex conditional logic, state transitions, or math operations instead of trivial getters/setters or standard derive macros. Only add table-driven tests for functions with complex decision logic. Ensure the tested code actually *does* something.
