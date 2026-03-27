## 2024-03-15 - [Clarified Audio Mixing priority]
**Confusion:** The documentation for `SfxMixer` and `AudioDriver` was purely mechanical, lacking a high-level explanation of *how* these components actually work together in real-time, especially regarding channel stealing.
**Clarification:** Added a module-level story explaining how `cpal` threads demand data from the two independent paths (`SfxMixer` and `MidiPlayer`). Added `## Examples` to show how to play a sound and then `play_on_channel` to avoid cluttering channels with rapid weapon sounds.

## 2024-06-25 - [Clarified doom-map Level representation and UDMF conversions]
**Confusion:** The `doom-map` crate's `udmf` module was missing examples showing how to parse a `TEXTMAP` string and convert it into flat arrays for the rest of the engine (`UdmfMap::into_level_data`). Additionally, `Level` struct methods lacked examples demonstrating how they're constructed from WAD structures and what happens when invariants fail (`Level::bsp` panics).
**Clarification:** Added `## Examples` with executable doc-tests showing text-to-map conversion. Clarified `Level::from_wad` and `Level::from_wad_stack` usage. Explicitly added a `## Panics` section to `Level::bsp` to highlight that invariant violations post-load signify logic bugs or memory corruption.

## 2024-05-18 - Internal modules showing up as broken links
**Confusion:** Rustdoc throws warnings when public items link to `pub(crate)` modules or items via intra-doc links, and changing those internal modules to `pub mod` exposes them to the crate's public API.
**Clarification:** To resolve private intra-doc link warnings while keeping the public API clean, the internal item should be made `pub mod` but annotated with `#[doc(hidden)]`. This allows intra-doc links to resolve successfully in documentation, but prevents the internal implementation details from cluttering the generated docs or being promoted as part of the public API surface.
