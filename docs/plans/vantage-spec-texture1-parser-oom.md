# 🔭 Vantage: Spec for TEXTURE1 Parser OOM Mitigation

## Context

The engine currently parses the TEXTURE1 lump by reading a 4-byte `num_textures` value and immediately allocating a vector with that capacity using `Vec::with_capacity(num_textures)`. If a malicious or fuzzed WAD file provides an extremely large value for `num_textures`, the engine attempts to allocate a massive amount of memory immediately, which leads to an Out-Of-Memory (OOM) panic and crashes the game.

This spec defines the "What" and the "Why" for mitigating this vulnerability without affecting normal gameplay.

## 👤 User Story

"As a Player, I want to be able to load custom WAD files without the game instantly crashing or consuming all my system memory due to malformed header data, so that I can safely explore community content."

## 🎯 The "So What?" Ask

**What business problem does this solve?**
Uncapped allocations driven by unverified input data are a classic Denial of Service (DoS) vulnerability. By trusting the `num_textures` field before verifying the actual size of the input data, the engine is fragile when handling untested or deliberately malformed WAD files. Fixing this issue improves engine stability, enhances user trust when loading third-party content, and prevents catastrophic crashes during fuzz testing or edge-case scenarios.

## ✅ Acceptance Criteria

### 1. Bounded Allocation
- **Success Metric:** The engine never allocates more memory for the texture list than is mathematically possible given the size of the provided data buffer.
- **Criteria:**
  - The initial capacity of the texture vector must be constrained. It should be the minimum of the parsed `num_textures` and a mathematically sound maximum capacity derived from the actual length of the lump data.
  - For example, if each texture entry requires at least 4 bytes of data, the capacity must be capped at `data.len() / 4`.

### 2. Graceful Error Handling
- **Success Metric:** Malformed WAD files with invalid `num_textures` values fail gracefully during parsing rather than crashing the process.
- **Criteria:**
  - The engine must successfully load valid WAD files (like doom1.wad) without regressions.
  - The engine must reject or safely truncate loading when `num_textures` exceeds the available buffer data, rather than panicking with an OOM error.

## 🚫 Out of Scope

- **Advanced Sandboxing:** We are fixing a specific allocation issue during parsing, not implementing a full sandboxed execution environment for untrusted WADs.
- **Texture Format Changes:** We are modifying how the texture list is allocated, not changing how individual textures are decoded or rendered.
- **Full WAD Header Verification:** This pass specifically targets the TEXTURE1 parser. A complete engine-wide audit of all lump parsers is a separate initiative.

## ⚖️ Gap Analysis

The current `TEXTURE1` parser in the `doom-wad` crate directly uses `Vec::with_capacity` using the raw `num_textures` value from the parsed lump header. This is a common Rust anti-pattern when dealing with external data. To fix this, the allocation logic needs to be updated to implement a safe upper bound based on the actual size of the data slice being parsed.
