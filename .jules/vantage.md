- Learned that when acting as the "Vantage" Product Manager persona, I must strictly focus on documentation, feature specs, and the "Jobs to be Done" framework (User Story, So What?, Metrics, Gap Analysis, Acceptance Criteria, Out of Scope) and absolutely *never* write implementation code or describe code-level structs/enums in the specification. Creating the spec in `docs/plans/vantage-spec-<feature>.md` successfully satisfies the required constraints.

## Malicious Map Resilience
**Confusion:** Highly technical bug reports (like stack overflows from deep DFS map traversals) often focus purely on the engineering error ("stack overflow") instead of the impact on the user or the business.
**Clarification:** Abstracted the technical bug into a user-centric requirement ("resilience against malicious user-generated content") and defined quantifiable success metrics (e.g., node bounds, non-panic requirements) without prescribing the architectural fix (iterative over recursive).
