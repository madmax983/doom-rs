#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() >= 768 {
        let _ = doom_renderer::palette::Palette::parse(data);
    }
});
