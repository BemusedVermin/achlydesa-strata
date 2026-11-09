use proptest::prelude::*;
use strata_kernel::Frac;

use super::*;

/// The default combat die: three hit faces and two block faces of six.
fn hits(dice: u32) -> DicePool {
    DicePool {
        dice,
        success: 3,
        faces: 6,
    }
}

fn blocks(dice: u32) -> DicePool {
    DicePool {
        dice,
        success: 2,
        faces: 6,
    }
}

fn chances(odds: &WoundOdds) -> Vec<Frac> {
    (0..=odds.max()).map(|n| odds.chance(n)).collect()
}

/// The exact sum of `fracs`.
fn sum(fracs: &[Frac]) -> Frac {
    let add = |a: Frac, b: &Frac| {
        Frac::new(
            a.numer() * b.denom() + b.numer() * a.denom(),
            a.denom() * b.denom(),
        )
    };
    fracs.iter().fold(Frac::ZERO, add)
}

#[test]
fn one_die_against_one_die() {
    // A hit is 1/2 and a block 1/3: one wound when the hit lands and the block does not.
    let odds = WoundOdds::of(hits(1), blocks(1)).unwrap();
    assert_eq!(chances(&odds), [Frac::new(2, 3), Frac::new(1, 3)]);
    assert_eq!(odds.weights(), (&[4i128, 2][..], 6));
    assert_eq!(odds.expected(), Some(Frac::new(1, 3)));
}

#[test]
fn two_dice_against_one_die() {
    // Hits: 0, 1, 2 with 1/4, 1/2, 1/4. Two wounds: 1/4 * 2/3 = 1/6. One wound: 1/4 * 1/3 +
    // 1/2 * 2/3 = 5/12. None: the rest, 5/12.
    let odds = WoundOdds::of(hits(2), blocks(1)).unwrap();
    let expected = [Frac::new(5, 12), Frac::new(5, 12), Frac::new(1, 6)];
    assert_eq!(chances(&odds), expected);
    assert_eq!(odds.at_least(1), Frac::new(7, 12));
    assert_eq!(odds.expected(), Some(Frac::new(3, 4)));
    assert_eq!(sum(&chances(&odds)), Frac::ONE);
}

#[test]
fn one_die_against_two_dice() {
    // A wound needs the hit and no block on either die: 1/2 * (2/3)^2 = 2/9.
    let odds = WoundOdds::of(hits(1), blocks(2)).unwrap();
    assert_eq!(chances(&odds), [Frac::new(7, 9), Frac::new(2, 9)]);
}

#[test]
fn an_empty_pool_deals_no_wounds() {
    let odds = WoundOdds::of(hits(0), blocks(3)).unwrap();
    assert_eq!(chances(&odds), [Frac::ONE]);
    let unopposed = WoundOdds::of(hits(2), blocks(0)).unwrap();
    let expected = [Frac::new(1, 4), Frac::new(1, 2), Frac::new(1, 4)];
    assert_eq!(chances(&unopposed), expected);
}

#[test]
fn certain_and_impossible_faces() {
    let always = DicePool {
        dice: 2,
        success: 6,
        faces: 6,
    };
    let never = DicePool {
        dice: 4,
        success: 0,
        faces: 6,
    };
    let odds = WoundOdds::of(always, never).unwrap();
    assert_eq!(chances(&odds), [Frac::ZERO, Frac::ZERO, Frac::ONE]);
    let blanked = WoundOdds::of(never, always).unwrap();
    assert_eq!(chances(&blanked), [Frac::ONE]);
}

#[test]
fn a_malformed_die_has_no_odds() {
    let faceless = DicePool {
        dice: 1,
        success: 0,
        faces: 0,
    };
    let too_many = DicePool {
        dice: 1,
        success: 7,
        faces: 6,
    };
    assert_eq!(WoundOdds::of(faceless, blocks(1)), None);
    assert_eq!(WoundOdds::of(hits(1), too_many), None);
}

#[test]
fn pools_too_large_for_exact_odds_have_none() {
    let thirds = |dice| DicePool {
        dice,
        success: 1,
        faces: 3,
    };
    // 2^30 * 3^20 is under the cap; 2^60 * 3^40 fits in 128 bits but not under the cap.
    assert!(WoundOdds::of(hits(30), thirds(20)).is_some());
    assert_eq!(WoundOdds::of(hits(60), thirds(40)), None);
    assert_eq!(WoundOdds::of(hits(1024), blocks(1024)), None);
}

#[test]
fn odds_near_the_cap_compare_without_overflow() {
    let thirds = DicePool {
        dice: 20,
        success: 1,
        faces: 3,
    };
    let odds = WoundOdds::of(hits(30), thirds).unwrap();
    let (_, total) = odds.weights();
    assert!(total > MAX_TOTAL / 4, "the total is near the cap: {total}");
    for n in 0..odds.max() {
        assert!(odds.at_least(n + 1) <= odds.at_least(n));
        assert!(odds.chance(n) <= Frac::ONE);
    }
    // About eight wounds are expected over a total near the cap, so the numerator passes it.
    assert_eq!(odds.expected(), None);
    let smaller = WoundOdds::of(hits(20), thirds).unwrap();
    let expected = smaller.expected().unwrap();
    assert!(expected > Frac::ZERO && expected < Frac::new(20, 1));
}

#[test]
fn a_percentage_is_for_display() {
    assert!((percent(Frac::new(1, 3)) - 33.333).abs() < 0.001);
}

proptest! {
    #[test]
    fn the_distribution_sums_to_exactly_one(
        a in 0u32..12, b in 0u32..12, ha in 0u32..7, hb in 0u32..7, fa in 1u32..7, fb in 1u32..7,
    ) {
        let attack = DicePool { dice: a, success: ha.min(fa), faces: fa };
        let defense = DicePool { dice: b, success: hb.min(fb), faces: fb };
        let odds = WoundOdds::of(attack, defense).unwrap();
        let (weights, total) = odds.weights();
        prop_assert_eq!(weights.iter().sum::<i128>(), total);
        prop_assert_eq!(sum(&chances(&odds)), Frac::ONE);
        prop_assert_eq!(odds.at_least(0), Frac::ONE);
        prop_assert!(odds.max() <= a as usize);
        for n in 0..odds.max() {
            prop_assert!(odds.at_least(n + 1) <= odds.at_least(n));
        }
    }

    #[test]
    fn more_attack_dice_never_lower_the_expected_wounds(a in 0u32..10, b in 0u32..10) {
        let fewer = WoundOdds::of(hits(a), blocks(b)).unwrap().expected().unwrap();
        let more = WoundOdds::of(hits(a + 1), blocks(b)).unwrap().expected().unwrap();
        prop_assert!(more >= fewer);
    }
}
