#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = doom_wad::WadFile::parse(data.to_vec());
});
