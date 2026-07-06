🧨 **The Trigger:** Out-of-bounds index query in `blockmap` linedef indexing (e.g. from corrupt saves, tampered maps, or chaotic actor movements) causing `.unwrap_or(0)` to silently fall back to offset 0 (the blockmap header) rather than an empty result.
📉 **The Stack Trace:** (Silent logic error / phantom geometry generation due to parsing header data as block offsets).
🧪 **Reproduction:** Call `block_linedefs(col, row)` with out of bounds `col` or `row`.
😈 **Comment:** You assumed the fallback would yield zeroes. You were wrong. It yielded the header, parsing arbitrary origin coordinates as linedefs and creating phantom geometry.
