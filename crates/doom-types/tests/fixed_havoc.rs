use doom_types::fixed::Fixed16_16;

#[test]
fn havoc_test_fixed_div_by_zero_no_panic() {
    let a = Fixed16_16::from_raw(9);
    let b = Fixed16_16::from_raw(0);
    let c = a / b;
    assert_eq!(c.raw(), i32::MAX);
}
