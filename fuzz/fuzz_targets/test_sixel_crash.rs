#![no_main]

use libfuzzer_sys::fuzz_target;
use doom_tui::sixel::encode_doom_sixel;
use doom_renderer::palette::PaletteLut;

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 { return; }
    let fb = vec![0; 1];
    let lut = PaletteLut::grayscale();

    let src_w = data[0] as usize;
    let dst_w = data[1] as usize;
    let src_h = data[2] as usize;
    let dst_h = data[3] as usize;

    // Call it with fuzz data
    encode_doom_sixel(&fb, &lut, 0, src_w, src_h, dst_w, dst_h, 10);
});
