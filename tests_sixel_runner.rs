use doom_tui::sixel::encode_doom_sixel;
use doom_renderer::palette::PaletteLut;

fn main() {
    let fb = vec![0; 1];
    let lut = PaletteLut::grayscale();
    let res = std::panic::catch_unwind(|| {
        encode_doom_sixel(&fb, &lut, 0, 0, 1, 10, 10, 10);
    });
    if res.is_err() {
        println!("PANIC TRIGGERED!");
    } else {
        println!("NO PANIC.");
    }
}
