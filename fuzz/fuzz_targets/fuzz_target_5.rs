#![no_main]
use doom_game::MapId;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    MapId::from_name(data);
});
