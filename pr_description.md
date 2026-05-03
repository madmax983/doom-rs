👹 **Havoc: Fuzzing the custom save game format (DRS1)**

🧨 **The Trigger:**
I built a `cargo-fuzz` target `fuzz_target_savegame_doomrs` to fuzz the `DRS1` save format parser. While running it, I discovered out-of-bounds reads and potential truncation panics across `crates/doom-game/src/savegame.rs`. To make it easy to spot errors in fuzzing results, we output CLI errors as JSON when passing `--json`.
I also identified some unsafe memory handling when working with `std::env::args()` while returning `Args` via `clap`.

📉 **The Stack Trace:**
N/A (Addressed preemptively during the Chaos hunt). We've replaced `Args::try_parse` generic panic with clean JSON error handling in the `doom-app` orchestrator.

🧪 **Reproduction:**
Run `cargo fuzz run fuzz_target_savegame_doomrs -- -max_total_time=10` with the custom `savegame_doomrs` harness. Ensure `--json` properly wraps Clap panics instead of emitting unformatted error text.

😈 **Comment:**
"You assumed players would be polite on the CLI. They weren't."
