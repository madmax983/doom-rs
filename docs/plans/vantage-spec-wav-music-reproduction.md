# 🔭 Vantage: Spec for WAV Music Reproduction

## Context

The engine currently provides a robust OPL2 MIDI synthesis backend for reproducing the original classic Doom soundtrack using the GENMIDI format. However, as the engine matures and modern source port features are considered, players increasingly desire the ability to play high-quality, pre-recorded audio tracks (like the popular modern instrumental or orchestral remixes of the Doom soundtrack).

This spec defines the "What" and the "Why" for introducing WAV audio reproduction for Doom music.

## 👤 User Story

"As a Player, I want to use high-fidelity WAV files for the game's music instead of standard MIDI synthesis, so that I can enjoy a modern, studio-quality soundtrack while playing."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
While OPL2 synthesis provides an authentic retro experience, the modern Doom community frequently uses custom WADs or music packs that replace MIDI tracks with high-quality recorded audio (MP3, OGG, or WAV). By failing to support digital audio tracks for music, we alienate users who prefer modern music packs and limit the engine's appeal. Supporting WAV reproduction acts as the foundational step toward full modern digital audio support, greatly expanding the engine's utility for the broader modding community.

## ✅ Acceptance Criteria

### 1. File Replacement and Detection
- **Success Metric:** The engine automatically detects and plays a WAV file if one is provided as a replacement for a specific map's music track.
- **Criteria:**
  - The engine must gracefully intercept the music loading process. If a digital audio track (WAV) is found instead of a MUS/MIDI lump for a given track name (e.g., `D_E1M1`), it must be prioritized.
  - The engine must support loading these replacement WAVs either directly from a PWAD or via a designated external music folder.

### 2. Seamless Playback and Looping
- **Success Metric:** WAV music must play continuously in the background, loop seamlessly, and respond to music volume controls.
- **Criteria:**
  - The digital track must loop infinitely while the player remains in the level.
  - The volume of the WAV track must be adjustable via the standard music volume slider in the game's options menu.
  - Changing maps must cleanly stop the current track and start the new track without audio pops or memory leaks.

### 3. Graceful Fallback
- **Success Metric:** The existing OPL2 MIDI synthesis remains fully functional as the default fallback.
- **Criteria:**
  - If no WAV file is provided for a given track, the engine must transparently fall back to the original MUS lump and synthesize it using the existing OPL2 backend.

## 🚫 Out of Scope

- **Compressed Audio Formats (MP3/OGG/FLAC):** To minimize dependencies and decoding complexity in Phase 1, we will strictly support uncompressed WAV files. Support for compressed formats is deferred.
- **Dynamic/Interactive Music:** Systems that dynamically change the music based on combat intensity or player health are out of scope. The music will remain a static background loop, just like the original MIDI tracks.
- **3D Positional Music:** Music remains a global stereo track; it does not have a spatial origin in the 3D world.

## ⚖️ Gap Analysis

The engine's `doom-audio` subsystem currently routes music playback exclusively through a MIDI sequencer (`MidiPlayer` and `MusScore`) which generates PCM samples via OPL2 emulation. We lack a parallel digital audio streaming pipeline for music. To bridge this gap, we must refactor the music mixer to support dual modes: it must either tick the MIDI synthesizer or stream samples from a decoded WAV buffer. Furthermore, the `WadStack` and asset loading logic must be updated to identify WAV headers and pass the raw digital audio data to the new streaming pipeline.
