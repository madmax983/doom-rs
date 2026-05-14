#![no_main]

use libfuzzer_sys::fuzz_target;
use doom_map::lumps::*;

fuzz_target!(|data: &[u8]| {
    let _ = Thing::parse_lump(data);
    let _ = Linedef::parse_lump(data);
    let _ = Sidedef::parse_lump(data);
    let _ = Vertex::parse_lump(data);
    let _ = Seg::parse_lump(data);
    let _ = Ssector::parse_lump(data);
    let _ = Node::parse_lump(data);
    let _ = Sector::parse_lump(data);

    // Pass a valid-looking sector count for reject
    if let Ok(sectors) = Sector::parse_lump(data) {
        let _ = Reject::parse_lump(data, sectors.len());
    } else {
        let _ = Reject::parse_lump(data, 10);
    }

    let _ = Blockmap::parse_lump(data);
});
