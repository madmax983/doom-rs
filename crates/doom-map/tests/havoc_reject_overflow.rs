#![allow(missing_docs)]
use doom_map::lumps::Reject;

#[test]
fn havoc_trigger_reject_overflow() {
    // 65536 * 65536 overflows a 32-bit usize.
    let n_sectors = 65536;
    // This should no longer panic, but return a BadRejectSize error instead.
    assert!(Reject::parse_lump(&[0; 8], n_sectors).is_err());
}

#[test]
fn havoc_trigger_reject_overflow_max() {
    let n_sectors = usize::MAX;
    assert!(Reject::parse_lump(&[0; 8], n_sectors).is_err());
}
