with open(".jules/atlas.md", "a") as f:
    f.write("\n## YYYY-MM-DD - Move Skill enum\n")
    f.write("**Tangle:** `doom-game` had circular dependencies `state -> spawn -> state` because `Skill` enum was in `spawn` but used by `GameState`.\n")
    f.write("**Blueprint:** Extracted `Skill` enum to a new `skill.rs` module in `doom-game` and updated imports.\n")
    f.write("\n## YYYY-MM-DD - Reduce pub visibility in binary\n")
    f.write("**Tangle:** `doom-app` (a binary crate) was exposing many types as `pub` internally (e.g. `pub struct Console`, `pub struct DemoPlaybackApp`, `pub enum AudioEvent`), creating a 'Leaky Abstraction' intent despite being fundamentally private.\n")
    f.write("**Blueprint:** Changed `pub` to `pub(crate)` across internal modules (`audio_system`, `cheats`, `console`, `demo_mode`, `net_mode`, `savegame`, `cogmind/*`) to tighten boundaries.\n")
