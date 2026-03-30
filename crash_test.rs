use doom_map::lumps::{Thing, Linedef, Sidedef, Vertex, Sector, Seg, Ssector, Node, Blockmap, Reject};

fn main() {
    let data = std::fs::read("fuzz/artifacts/lumps/oom-0d9cbd6bff1b6dfeb332f9ab44b4894ba1b5b77c").unwrap();
    let _ = Thing::parse_lump(&data);
    let _ = Linedef::parse_lump(&data);
    let _ = Sidedef::parse_lump(&data);
    let _ = Vertex::parse_lump(&data);
    let _ = Sector::parse_lump(&data);
    let _ = Seg::parse_lump(&data);
    let _ = Ssector::parse_lump(&data);
    let _ = Node::parse_lump(&data);
    let _ = Blockmap::parse_lump(&data);
}
