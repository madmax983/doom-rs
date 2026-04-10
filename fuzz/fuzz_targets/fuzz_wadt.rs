#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(wad) = doom_wad::WadFile::parse(data.to_vec()) {
        for lump in wad.lumps() {
            let lump_data = wad.lump_data(lump);
            if lump.name.as_str() == "PLAYPAL" {
                if lump_data.len() >= 768 {
                    let _ = doom_renderer::palette::PaletteLut::from_playpal(lump_data);
                }
            } else if lump.name.as_str() == "COLORMAP" {
                if lump_data.len() == 8704 {
                    let _ = doom_renderer::colormap::ColormapCache::from_test_data(lump_data.to_vec());
                }
            } else if lump.name.as_str() == "TEXTURE1" {
                let _ = doom_renderer::texture_compose::TextureDirectory::new(lump_data, None, &[0; 1000]);
            }
        }
    }
});
