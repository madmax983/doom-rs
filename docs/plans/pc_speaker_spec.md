# PC Speaker (DP*/PCSND) Support Specification

## User Story
As a vanilla Doom purist, I want PC Speaker sound support, so that I can experience the game exactly as it sounded on legacy hardware without modern digital audio cards.

## "So What?" (The Business Problem)
The primary value proposition of the `doom-rs` engine is feature parity with Chocolate Doom (vanilla accuracy). Currently, the engine lacks support for the fallback PC Speaker audio mode (`DP*` / `PCSND` lumps), which is a hard requirement for 100% vanilla fidelity. By implementing this, we eliminate a noted gap in our PARITY_ROADMAP.md and satisfy our core audience of retro enthusiasts who expect perfect legacy emulation.

## Success Metrics
- **Feature Parity:** 100% playback capability of all standard PC Speaker sound effects present in the original DOOM/DOOM2 WADs.
- **Audio Latency:** PC speaker emulation latency is consistent with digital SFX output (< 20ms).
- **Stability:** 0 crashes when switching between digital SFX and PC Speaker modes at runtime or via CLI.

## Gap Analysis
- **Current State:** The engine supports digital SFX via `cpal` and OPL2 FM synth for music. PC Speaker sounds are completely unsupported and ignored.
- **Standard Libs / Market:** Chocolate Doom and other vanilla ports accurately emulate the IBM PC speaker using square waves and standard lump parsing. We must replicate this capability.

## Acceptance Criteria
- Must parse and load `DP*` (Doom 1) and `PCSND` (Doom 2) lumps from standard WAD files.
- Must synthesize square wave audio approximating the original hardware's frequency output.
- Must support a CLI flag (e.g., `--pcsound`) to force PC speaker mode instead of digital SFX.
- Must gracefully fall back to silence if the lump is missing or corrupted, without panicking.

## Out of Scope
- Support for non-standard or extended PC speaker music (only sound effects are required).
- Advanced audio filtering or spatial 3D audio applied to PC speaker sounds.
- Native MIDI, Timidity, or GUS device support (these are separate roadmap items).
