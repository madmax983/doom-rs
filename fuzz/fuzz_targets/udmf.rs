#![no_main]

use doom_map::udmf::UdmfMap;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(map) = UdmfMap::parse(data) {
        let _ = map.into_level_data();
    }
});
