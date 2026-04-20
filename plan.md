1. **Target**: Address the "let Some(sd) = ...; let sector_idx = sd.sector" Boolean Blindness/Pyramid of Doom in `crates/doom-game/src/specials.rs`. The issue is deeply nested redundant lookup logic.
2. **Review other occurrences**: `grep` shows we have cleaned up `activate_doors`. Need to review the other `activate_*` methods just to be sure there are no remaining repetitive lookups for `left_sidedef`. Since `get_other.py` showed 0 matches, they might be fully clean or use another pattern.
3. **Run tests & clippy**: Ensure all tests still pass and clippy is clean.
4. **Complete Pre Commit Steps**: Follow `pre_commit_instructions` tool and perform all necessary verification and reflections.
5. **Create PR**: Present a PR following the Forge persona guidelines.
