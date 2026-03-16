## 2024-03-15 - [Clarified Audio Mixing priority]
**Confusion:** The documentation for `SfxMixer` and `AudioDriver` was purely mechanical, lacking a high-level explanation of *how* these components actually work together in real-time, especially regarding channel stealing.
**Clarification:** Added a module-level story explaining how `cpal` threads demand data from the two independent paths (`SfxMixer` and `MidiPlayer`). Added `## Examples` to show how to play a sound and then `play_on_channel` to avoid cluttering channels with rapid weapon sounds.
