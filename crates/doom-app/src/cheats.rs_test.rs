use doom_app::cheats::CheatDetector;

#[test]
fn test_multibyte() {
    let mut det = CheatDetector::new();
    det.feed('日'); // Multi-byte character
    det.feed('本'); // Multi-byte character
    det.feed('語'); // Multi-byte character
}
