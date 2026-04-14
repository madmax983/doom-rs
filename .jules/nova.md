**Integrate StyleMeter into CogmindHud**
**Learning:** doom-game implements an optional feature `style_meter` which tracks DMC-like style. However, the score isn't visibly exposed to the user. Integrating it with doom-tui's CogmindHud provides an awesome way to utilize this hidden feature.
**Action:** Plumbed `StyleRank` up through `GameState`, `CogmindHud`, and `CogmindHudWidget`.
