use strata_kernel::DefId;

use super::*;
use crate::state::UnitState;

/// The fixture unit with `n` copies of its one ability.
fn unit_with(n: usize) -> UnitDef {
    let json = include_str!("../../tests/fixtures/unit.json");
    let mut def: UnitDef = serde_json::from_str(json).unwrap();
    let one = def.abilities[0].clone();
    def.abilities = vec![one; n];
    def
}

fn state_with_unit(def: DefId) -> GameState {
    let mut state = GameState::default();
    state.players.push(crate::state::PlayerState::default());
    state.units.push(UnitState {
        def,
        ..UnitState::default()
    });
    state
}

#[test]
fn each_printed_entry_is_attached_to_the_unit_by_its_index() {
    let state = state_with_unit(DefId(7));
    let def = unit_with(2);
    let events = state.printed_ability_events(UnitId(0), |_| Some(&def));
    let keys: Vec<_> = events
        .iter()
        .map(|e| match e {
            Event::AbilityAttached { def, owner, params } => {
                assert_eq!(*owner, AbilityOwner::Unit(UnitId(0)));
                assert!(params.is_empty());
                *def
            }
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    let key = |index| AbilityKey::Unit {
        unit: DefId(7),
        index,
    };
    assert_eq!(keys, [key(0), key(1)]);
}

#[test]
fn applying_the_events_twice_attaches_each_entry_once() {
    let mut state = state_with_unit(DefId(7));
    let def = unit_with(2);
    for _ in 0..2 {
        for event in state.printed_ability_events(UnitId(0), |_| Some(&def)) {
            state.apply(&event).unwrap();
        }
    }
    assert_eq!(state.abilities.len(), 2);
    assert!(
        state
            .printed_ability_events(UnitId(0), |_| Some(&def))
            .is_empty()
    );
}

#[test]
fn a_unit_the_state_does_not_have_gets_nothing() {
    let state = GameState::default();
    let def = unit_with(1);
    let events = state.printed_ability_events(UnitId(3), |_| Some(&def));
    assert!(events.is_empty());
}

#[test]
fn the_definition_is_looked_up_by_the_units_recorded_def() {
    let state = state_with_unit(DefId(7));
    let def = unit_with(1);
    let mut asked = None;
    let events = state.printed_ability_events(UnitId(0), |id| {
        asked = Some(id);
        Some(&def)
    });
    assert_eq!(asked, Some(DefId(7)));
    assert_eq!(events.len(), 1);
    // A definition the lookup does not know attaches nothing.
    assert!(state.printed_ability_events(UnitId(0), |_| None).is_empty());
}
