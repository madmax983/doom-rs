🎯 **Target:** Functions within `doom-renderer` such as `column::draw_column`, `span::draw_span`, `fuzz::draw_fuzz_column`, `sprite_clip::SpriteClipHistory::push`, `wad_font::WadFont::string_width`, and `flat_cache::FlatCache::load_from_stack_with_profile`.

💣 **Risk:** These are core rendering loop functions where off-by-one or out-of-bounds inputs could cause runtime panics or subtle visual artifacts (e.g., fuzz effect breaking out of bounds or sprite clip lists overflowing). Unhandled fallback paths could cause panics.

🧪 **Strategy:**
- Added test cases `draw_column_out_of_bounds_x_returns_early` and `draw_span_out_of_bounds_y_returns_early` to ensure loops safely bypass execution rather than indexing out of bounds.
- Added test cases verifying boundary logic, such as `test_sprite_clip_history_overflow` and `test_record_sprite_clip_step_skips_duplicates`.
- Covered explicit compatibility configuration (`VanillaStrict` vs `Extended`) inside `test_flat_cache_load_from_stack_with_profile_extended`.
- Added test cases for rendering empty strings or strings with missing glyphs (`draw_string_ignores_spaces_and_advances_x` and `string_width_with_mixed_characters`).
- Verified `fuzz::draw_fuzz_column` logic successfully executes its fallback path fully and mutates memory exactly up to expected boundaries (`draw_fuzz_column_modifies_framebuffer_all_pixels`).

🔬 **Verification:** Run `cargo test --package doom-renderer` to verify tests pass and use `cargo llvm-cov --package doom-renderer --show-missing-lines` to see the newly improved coverage map.
