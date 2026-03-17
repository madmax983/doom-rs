#![no_main]

use doom_wad::WadFile;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(wad) = WadFile::parse(data.to_vec()) {
        for lump in wad.lumps() {
            let _ = wad.lump_data(lump);
        }
        let _ = wad.kind();
        let _ = wad.lump_count();
    }
});
