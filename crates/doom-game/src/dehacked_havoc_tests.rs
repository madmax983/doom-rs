use super::*;
#[test]
fn havoc_dehacked_nan() {
    let patch_text = "Patch File for DeHackEd v3.0\nDoom version = 19\nPatch format = 6\n\nThing 1\nHit points = NaN\n";
    let patch = DehPatch::parse(patch_text);
    assert!(patch.is_err(), "Expected an error when parsing NaN, got {:?}", patch);
}
