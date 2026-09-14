//! Literal independent cases from research/journey-rules/expected-scenarios.json.
//! Arithmetic evidence only: no source content assembly or combat/death acceptance.

use clubscape_game_types::*;
use clubscape_world_engine::{RandomSource, random::chance_numerator, source_math::*};

#[test]
fn independently_authored_skilling_probability_vectors_include_rounding_and_plus_one() {
    for (level, low, high, expected) in [
        (1, 100, 350, 101),
        (2, 100, 350, 104),
        (3, 100, 350, 106),
        (1, 48, 256, 49),
        (1, 64, 200, 65),
        (3, 64, 200, 68),
        (1, 128, 512, 129),
        (1, 118, 492, 119),
        (1, 128, 512, 129),
        (99, 100, 350, 256),
        (1, 64, 512, 65),
    ] {
        assert_eq!(skilling_success_count(level, low, high).unwrap(), expected);
        let rule = ChanceRule {
            numerator_at_level_1: low + 1,
            numerator_at_level_99: high + 1,
            denominator: 256,
        };
        assert_eq!(chance_numerator(&rule, level).unwrap(), expected);
    }
}

#[test]
fn literal_probability_rules_do_not_add_one_twice_or_make_zero_chance_succeed() {
    for (count, expected) in [(0, 0), (1, 1), (49, 49), (101, 101), (500, 256)] {
        let rule = ChanceRule {
            numerator_at_level_1: count,
            numerator_at_level_99: count,
            denominator: 256,
        };
        assert_eq!(chance_numerator(&rule, 1).unwrap(), expected);
    }
    let descending = ChanceRule {
        numerator_at_level_1: 101,
        numerator_at_level_99: 3,
        denominator: 256,
    };
    assert_eq!(chance_numerator(&descending, 2).unwrap(), 100);
    assert_eq!(chance_numerator(&descending, 99).unwrap(), 3);
    assert!(chance_numerator(&descending, 100).is_err());
}

#[test]
fn independent_post_2025_energy_vectors_use_hundredths_and_weight_clamping() {
    assert_eq!(run_drain_units(1, 0).unwrap(), 59);
    assert_eq!(run_drain_units(1, 100_000).unwrap(), 126);
    assert_eq!(run_drain_units(1, -5000).unwrap(), 59);
    assert_eq!(run_recovery_units(1).unwrap(), 15);
}

#[test]
fn independent_melee_and_ranged_effective_and_maximum_hit_vectors() {
    assert_eq!(effective_level(1, 1, 1, 3).unwrap(), 12);
    assert_eq!(maximum_hit(9, 5).unwrap(), 1);
    assert_eq!(maximum_hit(12, 7).unwrap(), 1);
    assert_eq!(accuracy_fraction(816, 490), (571, 817));
    assert_eq!(accuracy_fraction(640, 640), (320, 641));
    assert_eq!(maximum_accuracy_roll(10, -100).unwrap(), 0);
}

#[test]
fn independent_magic_rolls_use_magic_not_physical_defence() {
    assert_eq!(
        maximum_accuracy_roll(effective_level(1, 1, 1, 0).unwrap(), 0).unwrap(),
        576
    );
    assert_eq!(maximum_accuracy_roll(1 + 9, -15).unwrap(), 490);
}

#[test]
fn inclusive_accuracy_draws_tie_as_a_miss() {
    struct Rolls(std::collections::VecDeque<u32>);
    impl RandomSource for Rolls {
        fn draw_below(&mut self, upper: u32) -> GameResult<u32> {
            let value = self.0.pop_front().unwrap();
            assert!(value < upper);
            Ok(value)
        }
    }
    let mut hit = Rolls([816, 490].into());
    assert!(opposed_accuracy(816, 490, &mut hit).unwrap());
    let mut tie = Rolls([490, 490].into());
    assert!(!opposed_accuracy(816, 490, &mut tie).unwrap());
}

#[test]
fn independent_damage_xp_and_nonfatal_constraints_are_not_guaranteed_damage() {
    assert_eq!(player_damage(false, 0, 1, 3).unwrap(), 0);
    assert_eq!(player_damage(true, 0, 1, 3).unwrap(), 1);
    assert_eq!(player_damage(true, 8, 8, 2).unwrap(), 2);
    assert_eq!(incoming_damage(3, 20, true), 2);
    assert_eq!(incoming_damage(1, 1, true), 0);
    assert_eq!(incoming_damage(3, 20, false), 3);
    assert_eq!(hitpoints_xp_tenths(2, false), 26);
    assert_eq!(hitpoints_xp_tenths(3, false), 40);
    assert_eq!(hitpoints_xp_tenths(3, true), 0);
}

#[test]
fn independent_stock_sensitive_general_store_prices_are_per_unit() {
    assert_eq!(general_store_buy_price(2, 3, 3), 2);
    assert_eq!(general_store_buy_price(2, 3, 1), 2);
    assert_eq!(general_store_sell_price(1, 5, 5), 0);
    assert_eq!(general_store_sell_price(26, 0, 0), 10);
    assert_eq!(general_store_sell_price(26, 0, 1), 9);
}

#[test]
fn independent_grave_and_office_fee_boundaries() {
    assert_eq!(
        grave_fee(&[99_999, 100_000, 999_999, 1_000_000, 9_999_999, 10_000_000]),
        122_000
    );
    assert_eq!(grave_fee(&[10_000_000; 6]), 500_000);
    assert_eq!(office_fee(&[99_999, 100_000]).unwrap(), 5000);
    assert_eq!(office_fee(&[100_019]).unwrap(), 5000);
}

#[test]
fn unsupported_formula_domains_and_overflow_are_errors() {
    assert!(skilling_success_count(0, 100, 350).is_err());
    assert!(run_drain_units(100, 0).is_err());
    assert!(effective_level(1, 1, 0, 0).is_err());
    assert!(maximum_hit(u32::MAX, i32::MAX).is_err());
    assert!(maximum_accuracy_roll(u32::MAX, i32::MAX).is_err());
    assert!(player_damage(true, 2, 1, 3).is_err());
    assert!(office_fee(&[u64::MAX; 21]).is_err());
}
