#![no_main]

use libfuzzer_sys::fuzz_target;
use doom_app::cheats::{apply_cheat, CheatDetector};

// We just want to ensure these don't panic on any arbitrary byte sequence
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // dummy game state wouldn't work easily here since it's hard to construct
        // Instead, let's just use CheatDetector since we saw the code
        let mut detector = CheatDetector::new();
        for c in s.chars() {
            detector.feed(c);
        }
    }
});
