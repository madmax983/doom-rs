👺 Havoc: Compile-time trig tables and Loom Deadlock testing

🧨 **The Trigger**
The `doom-types/src/angle.rs` module was generating its trig tables (`SINE_TABLE`) at runtime on the first access, guarded by a `static mut` and an `AtomicBool` via the `init_trig_tables()` function. This caused a massive proliferation of `unsafe { Bam::init_trig_tables() }` blocks sprinkled throughout `doom-app`, `doom-game`, `doom-renderer`, and `doom-tui`.

If a developer forgot to call it, it would panic or use undefined behavior. When run in a multi-threaded testing environment (like `cargo test`), the concurrent access to `init_trig_tables()` was an unprotected data race that could cause garbage data to be read.

📉 **The Stack Trace**
No immediate panics, but a looming threat of data races and memory corruption due to unchecked parallel initialization during integration tests.

🧪 **Reproduction**
Run a `loom::model` test checking for race conditions around `Bam::init_trig_tables()`, or simply attempt to run `cargo test --all-targets` with thread concurrency.

😈 **Comment**
You assumed the single-threaded context of the game loop would save you. You were wrong.

I generated a precomputed `[Fixed16_16; 8192]` static lookup table in python, saving it into `trig_table.rs`. Now, `Bam::sin()` and `Bam::cos()` are completely safe and pure functions. I removed all `unsafe { Bam::init_trig_tables() }` calls and wiped `static mut SINE_TABLE` from existence. Additionally, I added `havoc_loom_deadlock_test` to verify that `AudioDriver::mixer` cross-thread calls do not deadlock.
