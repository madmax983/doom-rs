#![no_main]
use libfuzzer_sys::fuzz_target;
use doom_wad::WadFile;

fuzz_target!(|data: &[u8]| {
    let _ = WadFile::parse(data.to_vec());
});
