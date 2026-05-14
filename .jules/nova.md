**Integrate StyleMeter into CogmindHud**
**Learning:** doom-game implements an optional feature `style_meter` which tracks DMC-like style. However, the score isn't visibly exposed to the user. Integrating it with doom-tui's CogmindHud provides an awesome way to utilize this hidden feature.
**Action:** Plumbed `StyleRank` up through `GameState`, `CogmindHud`, and `CogmindHudWidget`.
## YYYY-MM-DD - [Style-Driven AI Director]
**The Spark:** The `AiDirector` module provided some enum suggestions (`SpawnAmbush`, `SpawnRelief`) based on player health but wasn't wired to actually spawn anything into the world or interact with the `StyleMeter`.
**The Feature:** Implemented dynamic spawning logic in `tick_all_mobjs`. Wired the `AiDirector` to consider the `StyleMeter` rank to spawn ambushes when at high style (`SmokinSexyStyle` or `Sick`), or relief items when health is low and style is lagging. Used `point_in_subsector` logic to safely find sector floor heights and check `ceil_height` bounds before allocating `Mobj` structs into the `MobjSlab`.
**The Potential:** Opens up dynamic difficulty adjustment and creates a more rewarding gameplay loop where playing stylishly directly influences the challenge of the arena.
