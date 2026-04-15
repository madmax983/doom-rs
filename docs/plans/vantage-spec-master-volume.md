# 🔭 Vantage: Spec for Master Volume Control

## 👤 User Story
As a Player, I want to be able to control a single global master volume setting, so that I can easily mute or adjust the overall game audio without having to manually tweak both the Sound Volume and Music Volume independently.

## ❓ So What?
Currently, the audio system only supports independent volume settings for Sound Effects and Music. If a player receives a phone call, wants to quickly mute the game, or simply finds the overall application too loud relative to other apps on their system, they must dig into the Options menu and lower multiple sliders individually. This creates friction and a poor "Human Interface." A single master volume solves this by providing a convenient, one-stop control for all audio output.

## 📏 Metric Definition
- **Success Criteria:**
  - A new Master Volume setting is introduced that acts as a global multiplier (0% to 100%) applied to the final audio output mix.
  - The master volume correctly scales both Sound Effects and Music proportionally without altering their individual relative volume settings.
  - Exposing this setting in the configuration allows players to easily mute (0%) or adjust the entire game's audio output.

## 🔍 Gap Analysis
- **Current State:** The system handles Sound Effects and Music entirely independently, applying volume locally to individual sounds, but there is no overarching master stage applied to the final mixed output.
- **Standard Libs / Market:** Modern games, and even many source ports, universally provide a master volume slider in addition to categorized sliders (SFX, Music, Voice). Implementing a master multiplier at the very end of the signal chain is a standard architectural pattern for game audio engines.

## ✅ Acceptance Criteria
- Must introduce a global master volume setting.
- Must ensure that when master volume is 0%, the game outputs absolute silence without stopping the game.
- Must preserve the relative mix of Sound Effects and Music; the master volume should only scale the final audio just before it reaches the output device.

## 🚫 Out of Scope
- Adding new in-game UI menus for the Master Volume in this phase. The focus is on the backend audio pipeline functionality.
- Dynamic range compression or advanced master bus effects (e.g., limiters, EQs). The scope is strictly a linear gain multiplier.
