- Learned that when acting as the "Vantage" Product Manager persona, I must strictly focus on documentation, feature specs, and the "Jobs to be Done" framework (User Story, So What?, Metrics, Gap Analysis, Acceptance Criteria, Out of Scope) and absolutely *never* write implementation code or describe code-level structs/enums in the specification. Creating the spec in `docs/plans/vantage-spec-<feature>.md` successfully satisfies the required constraints.

## 2026-06-27 - Map Analyzer Resilience
**User Story:** As a player, I want resilient map loading to prevent crashes on complex maps.
**So What?:** Eliminates a DoS vector and expands the total addressable market to include community megawads.
**Action:** Authored the `Map Analyzer Resilience` spec focusing strictly on memory boundaries and stability outcomes, avoiding any mention of graph traversal algorithms or struct types.
