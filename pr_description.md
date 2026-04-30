⚒️ Forge: Resolve Boolean Blindness in hitscan_shot_angle

🚰 Smell: `hitscan_shot_angle` taking a boolean flag `accurate_first_shot` creates Boolean Blindness, hiding the intent of the parameter (`accurate_first_shot: true` vs `false`) at call sites in `weapon_fire.rs`.

✨ Solution: Replaced the boolean flag with a strongly typed enum `ShotAccuracy::Accurate` and `ShotAccuracy::Spread` to enforce correct usage at compile time and clarify intent at call sites. Also, shotgun spread calculations were cleanly refactored to use this function instead of duplicating math inline.

🧼 Benefit: Improves readability and intent clarification at call sites, making it explicit whether a shot should be accurate or use spread. Avoids duplication.

🛡️ Verification: Tests passed. No logic changed.
