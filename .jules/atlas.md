## Unify error handling into a central enum
**Tangle:** Duplicate `SaveError` enums existed in `doom-app` and `doom-game`, forcing manual conversions.
**Blueprint:** Delegated error propagation entirely to the `doom_game::savegame::SaveError` struct using the `#[thiserror::Error]` and `#[from]` macro inside `doom-app`'s `SaveError` enum.


**[Unify savegame error handling]**
**Tangle:** Duplicate `SaveError` enums existed in `doom-app` and `doom-game`, forcing manual conversions between identical error cases.
**Blueprint:** Delegated error propagation entirely to the `doom_game::savegame::SaveError` struct using the `#[thiserror::Error]` and `#[from]` macro inside `doom-app`'s `SaveError` enum.
