## 2024-05-18 - [Havoc: OOM on TEXTURE1 parser]
**Learning:** Doom's TEXTURE1 parsing uses a direct 4-byte `num_textures` read to allocate `Vec::with_capacity(num_textures)`. Fuzzing this length with large values triggers an immediate OOM.
**Action:** Use `.min(data.len() / 4)` to clamp lengths derived from WAD/lump headers, preventing massive allocations while still ensuring we parse valid entries up to the slice boundary.

* When fixing integer overflow vulnerabilities in data parsing or allocation sizing (e.g., calculating required bytes from header fields), use checked math operations like `.checked_mul()` paired with `.ok_or()` to return standard `Result::Err` values (e.g., `WadError::DirectoryOutOfBounds`) instead of allowing the thread to panic.
