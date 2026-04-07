#![no_main]
use libfuzzer_sys::fuzz_target;
use doom_game::savegame::load_game;

fuzz_target!(|data: &[u8]| {
    let _ = load_game(data);
});
