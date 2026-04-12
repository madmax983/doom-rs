use crate::player::PlayerState;
#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn player_math_cannot_panic(
            health in proptest::num::i32::ANY,
            amount in proptest::num::i32::ANY,
            cap in proptest::num::i32::ANY,
            ammo_type in proptest::num::usize::ANY,
            ammo_amount in proptest::num::u32::ANY,
            armor_type in proptest::num::u8::ANY,
        ) {
            let mut ps = PlayerState { health, ..Default::default() };

            // Check all mutating math functions for over/underflow
            ps.apply_damage(amount);
            ps.heal(amount);
            ps.heal_overheal(amount, cap);
            ps.set_health_capped(amount, cap);
            ps.give_armor(amount, armor_type);
            ps.deduct_armor(amount);
            ps.give_ammo(ammo_type % 4, ammo_amount);
            ps.use_ammo(ammo_type % 4, ammo_amount);
        }
    }
}
