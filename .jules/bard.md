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

## 2026-04-07 - [Internal modules showing up as broken links]
**Confusion:** Rustdoc throws warnings when public items link to `pub(crate)` modules or items via intra-doc links. Making those internal items `pub` with `#[doc(hidden)]` is an anti-pattern as it still exposes internal logic to the public API programmatically.
**Clarification:** To resolve private intra-doc link warnings while strictly keeping the public API clean, remove the intra-doc brackets (`[ ]`) and just use regular backticked markdown code formatting (e.g. `` `module_name` ``) for internal items in public docs.
## 2024-11-20 - [Clarified doom-app Orchestration and demo modes]
**Confusion:** The `doom-app` crate root (`main.rs`) and the `demo_mode` structs (`DemoRecordingWrapper`, `DemoPlaybackApp`) lacked narrative documentation explaining how the decoupled components form the "Grand Assembly" and how they intercept the game loop.
**Clarification:** Added a story-driven module-level `//!` block to `main.rs` detailing the application's orchestration role. Added executable `///` doc-tests to the demo wrappers to show how they initialize and pump the `DoomEventLoop` with `TicInput` without needing to trace into `demo_mode.rs`.
## 2024-05-18 - [Fixed broken doc tests by using exact enum paths]
**Confusion:** Doctests containing `ACTION_NONE` failed to compile because the symbol is not exported under `actions`.
**Clarification:** Modified doctests to correctly use `Action::NoAction as u8` when demonstrating dispatch behavior.
## 2024-05-18 - [Fixed TextureCache/FlatCache struct docs]
**Confusion:** The struct doc-comments in `TextureCache` and `FlatCache` were separated from their structs by `use` statements, causing rustdoc to not associate them correctly.
**Clarification:** Moved the `use` statements above the doc-comments and added executable `## Examples` sections that create an in-memory `WadFile` to satisfy the "executable examples" rule.

## 2024-04-17 - Added Doc Tests for Core Types
**Confusion:** Many public types in `doom-types` lacked examples, leaving users to infer how to use core mechanics like `Bam`, `Fixed16_16`, and `BBox`.
**Clarification:** Added explicit `## Examples` doc-tests for all major public structs and enums in `doom-types`, guaranteeing they are tested and compiled.
## 2024-04-19 - [Added doc tests for SoundRequest emitter functions]
**Confusion:** The `SoundRequest` type in `doom-game::state` lacked documentation and executable examples for `emitter` and `origin_handle`.
**Clarification:** Added explicit `///` block comments with `## Examples` doc-tests for both `emitter` and `origin_handle`. During testing, we encountered compilation errors regarding missing methods (`MobjHandle::from_index` and `Fixed16_16::from_f64`), so the examples were adjusted to use real working syntax (`MobjSlab::alloc` and `Fixed16_16::from_int`) to ensure accurate docs.

## 2024-05-31 - [Clarified missing documentation in doom-renderer's sprite_clip module]
**Confusion:** The `sprite_clip` module lacked documentation outlining its purpose to avoid heap allocations during BSP traversal, as well as executable examples for its functions (`new`, `push`, `last`, `iter`).
**Clarification:** Added module-level `//!` narrative detailing how `SpriteClipHistory` mitigates performance degradation in the hot render loop by avoiding `Vec` allocations. Provided executable doc-tests (`///`) with realistic usage examples for `SpriteClipHistory`'s methods.
