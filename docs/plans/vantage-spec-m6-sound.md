# 🔭 Vantage: Spec for Sound Completeness (M6)

## 👤 User Story
As a Retro Gamer playing Doom, I want to hear the original PC speaker sound effects and select my preferred MIDI device (like Native MIDI, Timidity, or GUS), so that I can experience the authentic soundscape of the DOS era.

## ❓ So What?
Currently, the engine relies heavily on OPL2 synthesis and lacks the raw, iconic PC speaker sounds (DP* lumps). PC Speaker support is a hallmark of vanilla compatibility (Milestone M6). Furthermore, tying players exclusively to OPL2 limits their ability to use high-quality SoundFonts or native MIDI hardware. Offering these choices directly impacts the nostalgic value and perceived fidelity of the product.

## 📏 Metric Definition
- **Success Criteria:**
  - Players can toggle PC speaker sound effects via configuration or command line.
  - PC speaker mode correctly outputs the distinct "beeps" derived from vanilla DP* lumps.
  - Players can choose their music output device from available system options (Native MIDI, Timidity, GUS, OPL2).

## 🔍 Gap Analysis
- **Current State:** The engine supports OPL2 synthesis via `doom-audio` but does not parse or playback PC speaker lumps, nor does it support alternative music backend selection.
- **Market:** Chocolate Doom and other vanilla ports provide robust support for PC speaker and various MIDI devices.
- **Standard Libs:** We may need to interact further with `cpal` or other audio libraries to support alternative MIDI outputs.

## ✅ Acceptance Criteria
- Must implement parsing and playback of DP* PC speaker lumps.
- Must provide configuration options to select the active sound effects device (Digital vs PC Speaker).
- Must provide configuration options to select the active music device (OPL2, Native MIDI, Timidity, GUS).

## 🚫 Out of Scope
- Building a from-scratch MIDI synthesizer. We should rely on system capabilities or established libraries for new MIDI backends.
- Advanced spatial audio or modern sound enhancements (e.g., reverb, EAX).
