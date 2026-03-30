#![no_main]

use libfuzzer_sys::fuzz_target;
use doom_game::savegame::load_game;

fuzz_target!(|data: &[u8]| {
    if data.len() > 1000 { return; }
    let _ = load_game(data);
});
