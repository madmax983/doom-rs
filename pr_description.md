🎯 **Target:** `doom-map::lumps::Blockmap::block_linedefs` and `doom-net::transport::tests`

💣 **Risk:** Using `.unwrap_or(0)` for an out of bounds query fallback in blockmaps returned an invalid slice iteration from offset 0, which could cause test/runtime panics when analyzing bad input arrays in binary lump parsing. Additionally, `.unwrap_err()` in tests obscures context when a test fails, leading to uninformative panic messages when an expected error branch isn't hit.

🧪 **Strategy:** Safely check `get(idx)` bounds and correctly enforce `pos >= data.len()` checks in blockmaps rather than falling back to 0. Replaced `.unwrap_err()` with `.expect_err("should fail on ...")` to provide actionable context upon test failure.

🔬 **Verification:** Run `cargo test -p doom-map` and `cargo test -p doom-net` to verify test execution and logic remains intact.
