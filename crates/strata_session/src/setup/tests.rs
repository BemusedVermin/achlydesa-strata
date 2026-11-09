//! Printed abilities at match setup, on the `cards` fixture: two seats with two soldiers each,
//! every soldier printing Shieldwall.

use std::collections::BTreeMap;
use std::path::PathBuf;

use strata_kernel::Sym;
use strata_model::{
    AbilityKey, AbilityOwner, ArmyEntry, ArmyList, FigureStatus, MatchSetup, PlayerId, PlayerSetup,
    RulesetDef, ScenarioDef, SeedSource, Text, UnitDef, UnitId,
};

use super::*;
use crate::pack_test::PackTests;

fn cards() -> PackTests {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/packs/cards");
    PackTests::load(&[root]).unwrap()
}

fn soldiers() -> Option<ArmyList> {
    Some(ArmyList {
        name: Text("test".into()),
        format: Sym::new("none"),
        packs: BTreeMap::new(),
        units: vec![ArmyEntry {
            unit: Sym::new("test_soldier"),
            copies: 2,
        }],
    })
}

fn duel(registry: &ContentRegistry) -> MatchSetup {
    let (ruleset, _) = find::<RulesetDef>(registry, "cards", "rs").unwrap();
    let (scenario, _) = find::<ScenarioDef>(registry, "cards", "duel").unwrap();
    let seat = |id: u8, slot: &str| PlayerSetup {
        id: PlayerId(id),
        slot: Sym::new(slot),
        team: id,
        army: soldiers(),
    };
    MatchSetup {
        packs: pack_identities(registry),
        ruleset,
        format: None,
        scenario,
        seed: SeedSource::Plain(3),
        players: vec![seat(0, "a"), seat(1, "b")],
    }
}

#[test]
fn setup_attaches_each_units_printed_abilities_once_while_its_figures_wait_in_reserve() {
    let tests = cards();
    let registry = &tests.registry;
    let state = build_match(registry, duel(registry)).unwrap().state;
    let (soldier, _) = find::<UnitDef>(registry, "cards", "test_soldier").unwrap();
    let key = AbilityKey::Unit {
        unit: soldier,
        index: 0,
    };
    let attached: Vec<_> = state.abilities.iter().map(|a| (a.def, a.owner)).collect();
    let expected: Vec<_> = (0..4)
        .map(|u| (key, AbilityOwner::Unit(UnitId(u))))
        .collect();
    assert_eq!(attached, expected);
    assert!(
        state
            .figures
            .iter()
            .all(|f| f.status == FigureStatus::Reserve)
    );
    // Asking again, as a later lifecycle point would, attaches nothing more.
    for unit in 0..4 {
        let events = state.printed_ability_events(UnitId(unit), |id| registry.get::<UnitDef>(id));
        assert!(events.is_empty());
    }
}

#[test]
fn setup_with_printed_abilities_is_deterministic() {
    let tests = cards();
    let registry = &tests.registry;
    let first = build_match(registry, duel(registry)).unwrap().state;
    let second = build_match(registry, duel(registry)).unwrap().state;
    assert_eq!(first.hash(), second.hash());
    assert_eq!(first, second);
}
