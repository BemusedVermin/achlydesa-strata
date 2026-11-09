use strata_kernel::DefId;

use super::*;

fn choice() -> TokenChoice {
    TokenChoice {
        hand: vec![DefId(1), DefId(2), DefId(0)],
        units: vec![UnitId(0), UnitId(2), UnitId(3)],
    }
}

fn put(pairs: &[(u32, u32)]) -> Vec<TokenPlacement> {
    let one = |(unit, token): &(u32, u32)| TokenPlacement {
        unit: UnitId(*unit),
        token: DefId(*token),
    };
    pairs.iter().map(one).collect()
}

#[test]
fn the_whole_hand_on_different_listed_units_fits() {
    assert_eq!(choice().check(&put(&[(3, 0), (0, 2), (2, 1)])), Ok(()));
}

#[test]
fn a_partial_hand_or_an_empty_one_does_not_fit() {
    let partial = choice().check(&put(&[(0, 1), (2, 2)]));
    assert_eq!(partial, Err(BadPlacement::WholeHand));
    assert_eq!(
        TokenChoice::default().check(&[]),
        Err(BadPlacement::WholeHand)
    );
}

#[test]
fn units_must_be_listed_and_distinct() {
    let unlisted = choice().check(&put(&[(1, 1), (0, 2), (2, 0)]));
    assert_eq!(unlisted, Err(BadPlacement::Unit));
    let twice = choice().check(&put(&[(0, 1), (0, 2), (2, 0)]));
    assert_eq!(twice, Err(BadPlacement::Unit));
}

#[test]
fn tokens_must_come_from_the_hand_once_each() {
    let foreign = choice().check(&put(&[(0, 1), (2, 2), (3, 7)]));
    assert_eq!(foreign, Err(BadPlacement::Token));
    let repeated = choice().check(&put(&[(0, 1), (2, 1), (3, 0)]));
    assert_eq!(repeated, Err(BadPlacement::Token));
}
