## 2024-03-15 - [Clarified Audio Mixing priority]
**Confusion:** The documentation for `SfxMixer` and `AudioDriver` was purely mechanical, lacking a high-level explanation of *how* these components actually work together in real-time, especially regarding channel stealing.
**Clarification:** Added a module-level story explaining how `cpal` threads demand data from the two independent paths (`SfxMixer` and `MidiPlayer`). Added `## Examples` to show how to play a sound and then `play_on_channel` to avoid cluttering channels with rapid weapon sounds.

## 2024-06-25 - [Clarified doom-map Level representation and UDMF conversions]
**Confusion:** The `doom-map` crate's `udmf` module was missing examples showing how to parse a `TEXTMAP` string and convert it into flat arrays for the rest of the engine (`UdmfMap::into_level_data`). Additionally, `Level` struct methods lacked examples demonstrating how they're constructed from WAD structures and what happens when invariants fail (`Level::bsp` panics).
**Clarification:** Added `## Examples` with executable doc-tests showing text-to-map conversion. Clarified `Level::from_wad` and `Level::from_wad_stack` usage. Explicitly added a `## Panics` section to `Level::bsp` to highlight that invariant violations post-load signify logic bugs or memory corruption.

## 2024-05-18 - Internal modules showing up as broken links
**Confusion:** Rustdoc throws warnings when public items link to `pub(crate)` modules or items via intra-doc links, and changing those internal modules to `pub mod` exposes them to the crate's public API.
**Clarification:** To resolve private intra-doc link warnings while keeping the public API clean, the internal item should be made `pub mod` but annotated with `#[doc(hidden)]`. This allows intra-doc links to resolve successfully in documentation, but prevents the internal implementation details from cluttering the generated docs or being promoted as part of the public API surface.
## 2026-03-30 - [Clarified UDMF Level representation and conversion]
**Confusion:** The `doom-map` crate's `udmf` module was missing module-level documentation explaining the high-level concept of what UDMF is, how it relates to the binary WAD format, and how the AST is 'compiled' down into fast arrays.
**Clarification:** Added a detailed module-level story (`//!`) tracing the data pipeline, and added executable `## Examples` to `UdmfMap::into_level_data` to show how the flexible AST is converted into the classic `Level` arrays. Also documented the `UdmfError` struct comprehensively.

## 2024-07-28 - [Clarified WAD lump discovery behavior]
**Confusion:** The documentation for `WadFile::find_lump` and `WadFile::lumps_between` lacked examples demonstrating the "last defined wins" principle and how markers like `F_START`/`F_END` actually encapsulate data. There were also no executable doc-tests showing case-insensitivity during WAD parsing or how `map_lump_group` pulls out a classic 10-lump map versus UDMF.
**Clarification:** Added executable `## Examples` to `find_lump`, `find_lump_data`, `lumps_between`, and `map_lump_group` in `doom-wad`, highlighting how the parser handles multiple lumps with identical names and how to extract data bounds using marker lumps.
## 2024-04-03 - [Missing Module Docs]
**Confusion:** Module `doom-map::graph` and `doom-types::proofs` had no module-level documentation.
**Clarification:** Add `//!` docstrings for missing modules.
