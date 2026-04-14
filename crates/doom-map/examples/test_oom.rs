//! Test for Out-Of-Memory (OOM) handling in the Blockmap parser.
use doom_map::lumps::Blockmap;

fn main() {
    let mut data = vec![0; 8];
    // x_count
    data[4] = 0xFF;
    data[5] = 0xFF;
    // y_count
    data[6] = 0xFF;
    data[7] = 0xFF;

    let _ = Blockmap::parse_lump(&data);
}
