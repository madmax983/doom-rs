**[TraceRay Allocation Removal]
**Learning:** Found a hot path in `trace_ray` allocating `vec![false; N]` per raycast. This was causing huge overhead during explosions (which fire multiple rays).
**Action:** Pre-allocate vectors in the caller and pass `&mut Vec` to `trace_ray`, reusing them via `.clear()` to eliminate repeated heap allocations.
