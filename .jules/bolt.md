**[Eliminating per-frame Vec allocation in TUI widget]
**Learning:** The TUI renderer `DoomFramebufferWidget` was allocating multiple `Vec` per frame (`x_map`, `y_top_map`, `y_bot_map`) to precompute coordinate scaling. This resulted in unnecessary heap allocations on the hot path (per frame) while yielding marginal benefits over directly calculating the simple integer math (`(c * fb_w) / term_w`) inside the loops.
**Action:** Replaced `.collect::<Vec<_>>()` calls with inline calculations directly in the rendering loops, completely avoiding the per-frame heap allocations.
**[Eliminating per-frame Vec allocation in TUI widget]
**Learning:** The TUI renderer `DoomFramebufferWidget` was allocating multiple `Vec` per frame (`x_map`, `y_top_map`, `y_bot_map`) to precompute coordinate scaling. This resulted in unnecessary heap allocations on the hot path (per frame) while yielding marginal benefits over directly calculating the simple integer math (`(c * fb_w) / term_w`) inside the loops.
**Action:** Replaced `.collect::<Vec<_>>()` calls with inline calculations directly in the rendering loops, completely avoiding the per-frame heap allocations.
