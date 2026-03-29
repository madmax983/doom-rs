#![no_main]
use doom_renderer::sprite_lookup::sprite_lump_name_str;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    if data.len() < 4 { return; }
    let sprite_name = &data[..4];
    let frame = data.as_bytes()[0];
    let rotation = data.as_bytes()[1];
    sprite_lump_name_str(sprite_name, frame, rotation);
});
