#[test]
fn havoc_blockmap_out_of_bounds_returns_empty() {
    use doom_map::lumps::Blockmap;
    let mut data = vec![0u8; 14];
    data[4..6].copy_from_slice(&1u16.to_le_bytes());
    data[6..8].copy_from_slice(&1u16.to_le_bytes());
    data[8..10].copy_from_slice(&5u16.to_le_bytes()); // offset
    data[10..12].copy_from_slice(&0u16.to_le_bytes()); // list start
    data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes()); // list end

    let blockmap = Blockmap::parse_lump(&data).unwrap();
    let mut it = blockmap.block_linedefs(5, 5);
    assert_eq!(it.next(), None, "out of bounds block should be empty");
}
