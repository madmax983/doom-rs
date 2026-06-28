👤 **User Story:**
As a Server Operator or Player, I want the map analyzer to process highly complex or maliciously crafted maps without crashing, so that the game remains stable regardless of the map's topology.

✅ **Acceptance Criteria:**
- Must analyze large maps (e.g., >10,000 linear segments) without crashing or abruptly terminating.
- Must correctly identify map features (e.g., chokepoints) on complex geometries identical to how it processes smaller maps.

🚫 **Out of Scope:**
- Modifying the underlying rules for what constitutes a map feature (e.g., redefining chokepoints).
- General performance optimizations unrelated to stability.
