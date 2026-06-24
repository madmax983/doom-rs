#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Wrap data into a WadFile with PNAMES lump to hit the parse_pnames OOM vulnerability
    let mut wad_data = vec![0u8; 12 + 16 + data.len()];
    wad_data[0..4].copy_from_slice(b"IWAD");
    let num_lumps: i32 = 1;
    wad_data[4..8].copy_from_slice(&num_lumps.to_le_bytes());
    let info_table_offset: i32 = 12 + data.len() as i32;
    wad_data[8..12].copy_from_slice(&info_table_offset.to_le_bytes());

    // Lump data
    wad_data[12..12+data.len()].copy_from_slice(data);

    // Directory entry (16 bytes)
    let dir_offset = 12 + data.len();
    wad_data[dir_offset..dir_offset+4].copy_from_slice(&12i32.to_le_bytes()); // filepos = 12
    let size = data.len() as i32;
    wad_data[dir_offset+4..dir_offset+8].copy_from_slice(&size.to_le_bytes()); // size
    wad_data[dir_offset+8..dir_offset+16].copy_from_slice(b"PNAMES\0\0");

    if let Ok(wad) = doom_wad::WadFile::parse(wad_data) {
        let _cache = doom_renderer::texture::TextureCache::load(&wad);
    }
});
