👹 Havoc: Infinite Loops

This PR addresses three distinct infinite loop vulnerabilities across the codebase discovered via fuzzing. These loops can be triggered by malformed input or edge cases, causing the engine to hang indefinitely.

### 🧊 The Trigger

1. **`doom-audio::mus::MusScore::parse`**: A MUS lump containing an endless stream of events (like `PlayNote`) without a terminal `ScoreEnd` (type 6) event.
2. **`doom-audio::midi::MidiPlayer::advance_samples`**: A stream of MIDI events all sharing a `0` delta time.
3. **`doom-map::analyzer::MapAnalyzer::chokepoints`**: A sector graph containing cycles (e.g., `0 <-> 1`, `1 <-> 2`, `2 <-> 0`).

### 📉 The Stack Trace

1. **`MusScore::parse`**: The parser gets stuck in a `loop { ... }` block that reads events sequentially. If the EOF is reached without `ScoreEnd`, it never breaks. Fuzzer hangs.
2. **`MidiPlayer::advance_samples`**: The sequencer loops through events aiming to satisfy a sample window. If events process with 0 delta, `next_event_tick` never advances past `sample_count_end`. The `loop { ... }` spins infinitely without generating audio.
3. **`MapAnalyzer::chokepoints`**: The iterative DFS algorithm for finding articulation points failed to properly break out of revisiting nodes when a cycle was encountered, leading to an infinite cycle in the backtracking `while` loop.

### 🔬 Reproduction

1. Run `cargo test --test havoc_mus_timeout` or `cargo fuzz run fuzz_target_6`.
2. Run `cargo test --test havoc_midi_timeout` or `cargo fuzz run fuzz_target_6`.
3. Run `cargo test --test havoc_chokepoints_timeout`.

### 😈 Comment

You assumed every valid-looking MUS stream would kindly end, time always moves forward, and graphs are always trees. In my world, it stands still, loops infinitely, and nothing is what it seems.
