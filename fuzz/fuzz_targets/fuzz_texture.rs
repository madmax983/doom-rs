#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = doom_renderer::texture_compose::parse_patch(data);
    let _ = doom_renderer::texture_compose::parse_texture_lump(data);
});
