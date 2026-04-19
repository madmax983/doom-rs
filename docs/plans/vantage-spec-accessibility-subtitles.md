# 🔭 Vantage: Spec for Accessibility Subtitles

## 👤 User Story
As a deaf or hard-of-hearing player, I want critical game audio cues (like monster wakeups, doors opening, or item pickups) to be displayed as visual subtitles on the HUD, so that I can play the game effectively without relying on sound.

## 🤷‍♂️ So What?
What business problem does this solve?
Doom relies heavily on spatial audio and sound cues to alert players to danger (e.g., a Pinky roaring behind you). Without these cues, the game becomes significantly more difficult and less accessible for deaf or hard-of-hearing players, or players in environments where audio cannot be used. Adding accessibility subtitles expands the playable audience and improves the user experience for everyone.

## 📊 Metric Definition
Success = 100% of critical gameplay sound cues (monster sight, monster attack, secrets, critical environment sounds) are successfully pushed to the subtitle UI layer during gameplay without dropping frames or exceeding a 10ms processing latency per tic.

## 🕳️ Gap Analysis
Currently, the `doom-game` crate pushes `SoundRequest` events to an audio queue, which are then dispatched to `cpal` for audio rendering. There is no visual representation of these events in the terminal UI. Modern source ports and games (like Minecraft) offer visual audio indicators. We need to intercept the existing `SoundRequest` pipeline and pipe those events into the `doom-tui` or `doom-renderer` layers as text.

## ✅ Acceptance Criteria
- Must render text subtitles on the screen when a `SoundRequest` is processed.
- Must include directional indicators (e.g., "[<] Imp Wakes Up", "[>] Door Opens") if spatial audio information is available.
- Must be configurable via a toggle in the settings or startup arguments (e.g., `--subtitles`).
- Subtitles must fade or disappear after a reasonable timeout (e.g., 2-3 seconds).

## 🚫 Out of Scope
- Full 3D directional radar (like Fortnite's visual sound effects ring). Phase 1 is text-only.
- Translating or subtitling non-critical ambient sounds.
- Modifying the core game logic to spawn visual entities in the 3D world.
