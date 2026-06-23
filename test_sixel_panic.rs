use doom_tui::sixel::encode_doom_sixel;
use doom_renderer::palette::PaletteLut;

fn main() {
    let fb = vec![0; 1];
    let lut = PaletteLut::grayscale();
    encode_doom_sixel(&fb, &lut, 0, 0, 1, 10, 10, 10);
}
