# 👺 Havoc: Savegame string parsing rejects legacy non-UTF8 bytes

## 🧨 The Trigger
A corrupted or legacy savegame containing non-UTF8 bytes in the `description` or `level_name` fields causes the savegame loader to hard-error with `SaveError::Truncated` due to strict `core::str::from_utf8` validation. Legacy WAD/Savegame formats often have lossy or non-standard byte sequences.

## 📉 The Stack Trace
(No literal panic trace, but instead of loading the rest of the valid game state, the loader short-circuits):
```
Err(SaveError::Truncated)
```

## 🧪 Reproduction
Run `cargo fuzz run fuzz_target_savegame_doomrs` which generates non-UTF8 strings inside the payload.
Alternatively, see the `havoc_test_lossy_utf8_description_loads_successfully` and `havoc_test_lossy_utf8_level_name_loads_successfully` tests in `crates/doom-game/src/savegame.rs`.

## 😈 Comment
You assumed text from the 90s would always be pristine UTF-8. You were wrong.
We now use `String::from_utf8_lossy` instead of rejecting the entire save file.
