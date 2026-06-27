⚒️ Forge: Refactor player view height to eliminate boolean blindness

🚮 Smell: `next_player_view_height` taking a `player_dead: bool` obscuring intent at the call site and creating Boolean Blindness.
✨ Solution: Replaced the boolean argument with a strictly typed `PlayerStatus::Alive`/`PlayerStatus::Dead` enum.
🧼 Benefit: Makes the call site self-documenting and enforces type safety over raw booleans.
🛡️ Verification: Tests passed. No logic changed. Silenced unrelated `ratatui` deprecation warnings in `doom-tui` to keep CI green.
