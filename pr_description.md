⚒️ Forge: Refactor monster action functions

🚮 Smell: `a_chase`, `p_new_chase_dir`, `a_look`, and `a_pain_attack` in `crates/doom-game/src/actions.rs` were deeply nested "God Functions" (over 100 lines long, with up to 5 levels of nesting). The logic for checking sight, creating objects, and doing movement was all mixed into large continuous blocks.
✨ Solution: Extracted logic into small, named helper functions such as `a_chase_try_look_for_target`, `a_chase_can_missile`, `p_new_chase_dir_get_target`, `p_new_chase_dir_candidates`, `a_look_check_sound_targets`, `a_pain_attack_get_lost_soul_count`, and `a_pain_attack_create_skull`. Replaced deeply nested blocks with guard clauses and early returns.
🧼 Benefit: Dramatically flattens the pyramid of doom, making the main functions strictly follow their intended step-by-step logic. The codebase is much easier to read and maintain.
🛡️ Verification: Tests passed. No logic changed.
(Feedback implemented: Retained exact original behavior by passing coordinates as `Fixed16_16` types instead of truncating them early with `.to_int()`, ensuring fractional positional precision logic remains identical.)
