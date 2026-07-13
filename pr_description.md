🎯 Target:
UDMF (Universal Doom Map Format) parser in `crates/doom-map/src/udmf.rs`.

💣 Risk:
Complex string formatting errors, bit-logic edge cases, and unexpected token bugs were failing silently or panic-ing under untested paths during map serialization/deserialization. In particular, the exponent parsing logic and boolean flag accumulations (which map text keywords like "twosided" and "skill1" back to vanilla Doom integer flags) were largely uncovered by existing suites.

🧪 Strategy:
Added a new, self-contained integration test file (`crates/doom-map/tests/test_udmf_uncovered.rs`) that runs edge-case UDMF payload strings through the parser.
- Validated block comments (`/* ... */`), line comments (`// ...`), and unterminated comment error propagation.
- Verified correct `ParseFailed` output on malformed numeric strings (e.g. `1.0e+`).
- Provided a fully populated `linedef` block and `thing` block to rigorously assert that vanilla Doom bitwise flags (`FLAG_SECRET`, `THING_FLAG_EASY`, etc) accumulate correctly based on individual text boolean keys.

🔬 Verification:
Execute `cargo test -p doom-map --test test_udmf_uncovered`
