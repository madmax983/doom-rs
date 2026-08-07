# M6 Sound Completeness Specification

## The "So What?" (Business Problem)
Without PC speaker support and configurable MIDI devices, doom-rs fails the strict vanilla-parity requirement against Chocolate Doom. Providing these options is critical for adoption by purists and historically-accurate preservationists.

## Gap Analysis
The current engine provides `cpal` digital SFX and a from-scratch OPL2 FM synth. However, it lacks PC speaker (DP*/PCSND) emulation and device selection (native-MIDI/Timidity/GUS).

👤 **User Story:**
As a vanilla purist, I want PC speaker emulation and MIDI device selection, so that I can experience Doom's audio exactly as it sounded on my specific legacy DOS hardware.

✅ **Acceptance Criteria:**
- Must emulate the PC speaker by parsing `DP*` lumps to generate standard square-wave vanilla beeps.
- Must allow users to select their music device (OPL2, native-MIDI, Timidity, GUS).
- Success Metric = 100% audio parity with Chocolate Doom's PC speaker mode.

🚫 **Out of Scope:**
- HRTF, 3D EAX environmental audio, or HD audio remasters.
