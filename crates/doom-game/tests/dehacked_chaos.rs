use doom_game::dehacked::DehPatch;

#[test]
fn chaos_dehacked_overflow() {
    let input = "Text 18446744073709551615 18446744073709551615\nhello";
    let _ = DehPatch::parse(input);
}

#[test]
fn chaos_dehacked_overflow_2() {
    let input = "Text 18446744073709551615 100\nhello";
    let _ = DehPatch::parse(input);
}

#[test]
fn chaos_dehacked_out_of_bounds() {
    // If old_len + new_len does not overflow, we might slice out of bounds of `remaining`
    // Wait, the parsing is:
    // let old_text = remaining[..old_len].to_owned();
    // This will panic if remaining.len() < old_len.
    // Let's test this
    let input = "Text 50 50\nhi";
    let _ = DehPatch::parse(input);
}

#[test]
fn chaos_dehacked_out_of_bounds_2() {
    // If old_len + new_len does not overflow, we might slice out of bounds of `remaining`
    // Let's test this
    let input = "Text 50 50\n";
    let _ = DehPatch::parse(input);
}

#[test]
fn chaos_dehacked_out_of_bounds_3() {
    let input = "Text 2 4\nhi";
    let _ = DehPatch::parse(input);
}

use proptest::prelude::*;

proptest! {
    #[test]
    fn chaos_dehacked_proptest_no_panic(s in "\\PC*") {
        let _ = DehPatch::parse(&s);
    }
}
