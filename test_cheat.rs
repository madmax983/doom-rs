mod cheats {
    include!("crates/doom-app/src/cheats.rs");
}
fn main() {
    let mut det = cheats::CheatDetector::new();
    det.feed('日'); // Multi-byte character
    det.feed('本'); // Multi-byte character
    det.feed('語'); // Multi-byte character
}
