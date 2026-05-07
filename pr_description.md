**The Trigger:** Running Fuzzing across `doom-audio` components.
**The Stack Trace:**
```
thread 'havoc_repro' panicked at crates/doom-audio/src/midi.rs:559:28:
index out of bounds: the len is 578 but the index is 578
```
**Reproduction:**
Running focused resampling test in `havoc_repro.rs` using original values extracted from a `cargo fuzz` crash. The issue triggers when `max_needed` calculation rounds down improperly for interpolation boundaries.

**Comment:** "You thought your resampling logic was robust against arbitrary input rates. You were wrong."
