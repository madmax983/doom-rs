use doom_game::dehacked::DehPatch;
use proptest::prelude::*;

proptest! {
    #[test]
    fn parse_does_not_panic(s in "\\PC*") {
        let _ = DehPatch::parse(&s);
    }
}
