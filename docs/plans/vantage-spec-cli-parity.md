# 🔭 Vantage: Spec for Vanilla CLI Parity

## 👤 User Story
As a veteran Doom player, I want to use standard vanilla command-line flags (like `-nomonsters`, `-respawn`, and `-fast`) to launch the game, so that I can easily apply my familiar gameplay modifiers and execute speedrunning or custom challenge rulesets without learning a new configuration syntax.

## 💼 The "So What?" (Business Problem)
Friction at launch kills user engagement. Players who have spent decades typing `doom -warp 1 -skill 4 -fast -nomonsters` expect those exact commands to work out-of-the-box. While doom-rs parses these arguments today (and translates them to long-form flags like `--nomonsters`), they are not fully wired into the gameplay simulation (e.g., `spawn_level_things` currently ignores the `nomonsters` flag). Failing to implement these gameplay modifiers breaks parity with Chocolate Doom (Milestone M5), invalidates demo playback for runs recorded with these flags, and limits the utility of the engine for the speedrunning community.

## 📊 Metric Definition
- **Success** = 100% of standard vanilla Doom CLI gameplay modifier flags (`-nomonsters`, `-respawn`, `-fast`) successfully alter the gameplay state exactly as they do in Chocolate Doom.
- **Success** = Demo recording and playback accurately captures and replays sessions launched with these CLI modifiers.

## 🕳️ Gap Analysis
- **Current State:** The CLI parses flags like `-nomonsters`, `-respawn`, and `-fast` via `clap`, and demo headers track the `nomonsters` bit. However, a warning explicitly states that these flags are NOT wired into `spawn_level_things`. The gameplay logic ignores them.
- **Market Standard (Chocolate Doom):** The `-nomonsters` flag suppresses the spawning of most non-player entities, `-fast` drastically speeds up monster reaction times and projectile speeds (equivalent to Nightmare skill speed without respawning), and `-respawn` causes monsters to resurrect after a set delay.
- **The Gap:** The engine lacks the internal plumbing to pass these parsed configuration flags from the application initialization layer (`doom-app`) down to the map spawning and gameplay logic layers (`doom-game`).

## ✅ Acceptance Criteria
1. **Modifier `-nomonsters`:** When launched with `-nomonsters`, the game must not spawn any monsters during map initialization (with exceptions for specific vanilla edge cases, if any).
2. **Modifier `-fast`:** When launched with `-fast`, monster speed, reaction times, and projectile speeds must match Nightmare difficulty settings.
3. **Modifier `-respawn`:** When launched with `-respawn`, killed monsters must resurrect after the standard vanilla time delay.
4. **Demo Parity:** Demos recorded with any combination of these flags must correctly flag the demo header and replay with the exact same modifiers active, maintaining bit-for-bit demo sync.
5. **No Regression:** Standard game launches without these flags must behave exactly as they do today.

## 🚫 Out of Scope
- Implementing `-deathmatch` or `-altdeath` (multiplayer modifiers are part of the separate M8 Networking milestone).
- Full Vanilla savegame format compatibility (M7 milestone).
- Providing an in-game graphical menu to toggle these flags (this spec covers the CLI/simulation wiring only).
