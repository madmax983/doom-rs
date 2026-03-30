#![no_main]

use doom_map::lumps::{
    Blockmap, Linedef, Node, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = Thing::parse_lump(data);
    let _ = Linedef::parse_lump(data);
    let _ = Sidedef::parse_lump(data);
    let _ = Vertex::parse_lump(data);
    let _ = Sector::parse_lump(data);
    let _ = Seg::parse_lump(data);
    let _ = Ssector::parse_lump(data);
    let _ = Node::parse_lump(data);
    let _ = Blockmap::parse_lump(data);
    // Arbitrary number of sectors
    let _ = Reject::parse_lump(data, 100);
});
