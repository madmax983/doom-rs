🚷 Smell:
The codebase had multiple clippy warnings, such as obfuscated if/else chains, boolean blindness, unused mutable bindings, manual range contains, and unneeded `unsafe` blocks around safe functions like `doom_types::Bam::init_trig_tables()`.

✨ Solution:
Fixed all clippy lints across the crate by replacing `match` with `if let`, flattening identical `if-else` blocks, directly returning evaluated expressions instead of needless bools, using `(a..=b).contains`, adding clarity to operator precedence, and stripping unnecessary `unsafe` keywords and unused mutability keywords.

🧼 Benefit:
Enforces idiomatic Rust, improves code readability, prevents obscure precedence bugs, and drops technical debt across the crate without changing runtime behavior.

🛡️ Verification:
Tests passed. No logic changed. All clippy warnings were resolved.
