use strata_kernel::DefId;
use strata_model::{Command, FigureId, PlayerId, TokenChoice, TokenPlacement, UnitId};

use super::*;
use crate::input::pick::{Pick, Refusal};
use crate::input::tests::{figures, space, state};

/// Player 0 holds a 1 and a decoy, for unit 0 (figures 0 and 1) and a second unit 2 with no
/// figures in the fixture state.
fn placer() -> TokenPlacer {
    let choice = TokenChoice {
        hand: vec![DefId(1), DefId(0)],
        units: vec![UnitId(0), UnitId(2)],
    };
    TokenPlacer::new(PlayerId(0), choice)
}

fn placed(pairs: &[(u32, u32)]) -> Option<Command> {
    let one = |(unit, token): &(u32, u32)| TokenPlacement {
        unit: UnitId(*unit),
        token: DefId(*token),
    };
    Some(Command::PlaceTokens(pairs.iter().map(one).collect()))
}

#[test]
fn the_first_token_starts_selected_and_its_units_are_highlighted() {
    let s = state();
    let placer = placer();
    assert_eq!(placer.selected(), Some(0));
    assert_eq!(placer.assigned(), [None, None]);
    assert_eq!(placer.highlights(&s).figures, figures(&[0, 1]));
    assert_eq!(placer.command(), None);
}

#[test]
fn clicking_figures_places_the_hand_and_builds_the_placement() {
    let s = state();
    let mut placer = placer();
    assert_eq!(placer.click(&s, &Pick::at(&s, space(1))), Ok(()));
    assert_eq!(placer.assigned(), [Some(UnitId(0)), None]);
    assert_eq!(
        placer.selected(),
        Some(1),
        "the next token in hand is selected"
    );
    assert_eq!(placer.command(), None, "the decoy is still in hand");
    assert_eq!(placer.assign(UnitId(2)), Ok(()));
    assert_eq!(placer.selected(), None);
    assert!(placer.highlights(&s).figures.is_empty());
    assert_eq!(placer.command(), placed(&[(0, 1), (2, 0)]));
}

#[test]
fn a_unit_takes_one_token_so_moving_another_there_sends_the_first_back() {
    let mut placer = placer();
    placer.assign(UnitId(0)).unwrap();
    placer.assign(UnitId(2)).unwrap();
    assert_eq!(placer.assign(UnitId(0)), Err(Refusal::PickToken));
    placer.select(1);
    placer.assign(UnitId(0)).unwrap();
    assert_eq!(placer.assigned(), [None, Some(UnitId(0))]);
    assert_eq!(placer.selected(), Some(0));
    placer.assign(UnitId(2)).unwrap();
    assert_eq!(placer.command(), placed(&[(2, 1), (0, 0)]));
    placer.clear();
    assert_eq!(placer.assigned(), [None, None]);
    assert_eq!(placer.selected(), Some(0));
}

#[test]
fn refused_clicks_name_their_reason() {
    let s = state();
    let mut placer = placer();
    let click = |p: &mut TokenPlacer, q| p.click(&s, &Pick::at(&s, space(q)));
    assert_eq!(click(&mut placer, 2), Err(Refusal::NoFigure));
    assert_eq!(click(&mut placer, 3), Err(Refusal::NotYours));
    assert_eq!(placer.assign(UnitId(1)), Err(Refusal::NoTokenHere));
    assert_eq!(
        placer.click(&s, &Pick::figure(FigureId(9))),
        Err(Refusal::NoFigure)
    );
    assert_eq!(placer.assigned(), [None, None]);
}

#[test]
fn only_a_placement_the_rules_accept_becomes_a_command() {
    let choice = TokenChoice {
        hand: vec![DefId(1), DefId(0)],
        units: vec![UnitId(0), UnitId(2)],
    };
    let mut placer = TokenPlacer::new(PlayerId(0), choice.clone());
    placer.assign(UnitId(2)).unwrap();
    placer.assign(UnitId(0)).unwrap();
    let Some(Command::PlaceTokens(put)) = placer.command() else {
        panic!("the whole hand is placed");
    };
    assert_eq!(choice.check(&put), Ok(()));
}
