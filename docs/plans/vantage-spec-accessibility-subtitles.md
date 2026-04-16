# 🔭 Vantage: Spec for Accessibility Subtitles (Visual Audio Cues)

## 👤 User Story
As a Deaf or Hard-of-Hearing Player (or a player with their volume muted), I want on-screen text cues when important sounds play, so that I can react to enemies, items, and environment changes without relying on audio.

## ❓ So What?
Doom relies heavily on audio feedback. Monster roars, weapon pickups, and secret doors opening are critical survival cues. Without audio, players are at a severe disadvantage, making the game inaccessible to the deaf community and frustrating for users playing in quiet environments. Subtitles solve this by bridging the gap between audio events and visual feedback, making the engine vastly more inclusive. Complexity is minimal, utility is massive.

## 📏 Metric Definition
- **Success Criteria:**
  - Subtitles appear clearly in the UI when critical SFX events are triggered.
  - Subtitles fade or disappear after a short delay to prevent screen clutter.
  - 100% of major gameplay cues (Monster Alert, Item Pickup, Door Opening, Player Damage) are mapped to a visual subtitle.

## 🔍 Gap Analysis
- **Current State:** The engine triggers audio exclusively via the `doom-audio` subsystem and `SfxMixer`. There is currently no mechanism to broadcast these audio events back to the UI/HUD.
- **Standard Libs / Market:** Modern games universally offer closed captioning or directional audio subtitles (e.g., Minecraft, Half-Life 2). This is a standard accessibility feature expected in modern ports.

## ✅ Acceptance Criteria
- Must introduce a toggleable option in the Configuration File or command line to enable Subtitles.
- Must display brief, descriptive text (e.g., "*[Imp Roars]*", "*[Medkit Picked Up]*") when the corresponding SFX plays.
- Must ensure subtitles do not overlap and obscure critical gameplay HUD elements.
- Must queue or cleanly replace rapid, overlapping sounds so the UI remains readable.

## 🚫 Out of Scope
- Directional indicators (e.g., arrows pointing to where the sound came from) are out of scope for Phase 1.
- Subtitling every single footstep or ambient noise; focus only on critical gameplay events.
- Advanced font customization for the subtitles.
