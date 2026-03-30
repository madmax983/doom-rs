#![no_main]
use doom_game::savegame::load_game;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = load_game(data);
});
