#![no_main]

use doom_map::udmf::UdmfMap;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = UdmfMap::parse(data);
});
