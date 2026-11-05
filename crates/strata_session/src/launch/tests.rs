use std::collections::BTreeMap;
use std::path::PathBuf;

use std::time::Duration;
use strata_kernel::Sym;
use strata_model::{ArmyEntry, ArmyList, Text};

use crate::tests::attack;
use crate::*;

const WAIT: Duration = Duration::from_secs(60);

fn army(unit: &str) -> Option<ArmyList> {
    Some(ArmyList {
        name: Text("test".into()),
        format: Sym::new("none"),
        packs: BTreeMap::new(),
        units: vec![ArmyEntry {
            unit: Sym::new(unit),
            copies: 2,
        }],
    })
}

fn spec(scenario: &str, ruleset: Option<&str>) -> LaunchSpec {
    let root = env!("CARGO_MANIFEST_DIR");
    LaunchSpec {
        roots: vec![PathBuf::from(root).join("../../fixtures/packs/toy")],
        scenario: scenario.into(),
        ruleset: ruleset.map(str::to_owned),
        seed: 7,
        armies: vec![army("test_soldier"), army("test_archer")],
        commands: 1,
    }
}

fn first_error(spec: LaunchSpec) -> String {
    let handle = launch(spec);
    match handle.recv_timeout(WAIT) {
        Ok(SimMessage::Error(error)) => error.to_string(),
        _ => panic!("expected a launch error"),
    }
}

#[test]
fn a_launched_match_plays_the_commands_the_client_sends() {
    let handle = launch(spec("duel", Some("toy_rust")));
    let view = handle.recv_timeout(WAIT);
    assert!(matches!(&view, Ok(SimMessage::View(v)) if v.state.command_index == 0));
    handle.send_command(attack(0, 2));
    assert!(matches!(handle.recv_timeout(WAIT), Ok(SimMessage::Events(e)) if !e.is_empty()));
    assert!(
        matches!(handle.recv_timeout(WAIT), Ok(SimMessage::View(v)) if v.state.command_index == 1)
    );
    assert!(matches!(
        handle.recv_timeout(WAIT),
        Ok(SimMessage::Finished)
    ));
    assert_eq!(handle.shutdown(), Ok(()));
}

#[test]
fn launching_twice_with_the_same_commands_gives_the_same_hash() {
    let hash = || {
        let handle = launch(spec("duel", Some("toy_rust")));
        handle.send_command(attack(0, 2));
        let mut last = None;
        while let Ok(message) = handle.recv_timeout(WAIT) {
            match message {
                SimMessage::View(view) => last = Some(view.state.hash()),
                SimMessage::Finished => break,
                _ => {}
            }
        }
        last.unwrap()
    };
    assert_eq!(hash(), hash());
}

#[test]
fn a_launch_that_cannot_start_reports_why() {
    let got = first_error(spec("nowhere", Some("toy_rust")));
    assert!(got.contains("unknown scenario `nowhere`"), "{got}");
    let got = first_error(spec("duel", Some("nothing")));
    assert!(got.contains("unknown ruleset `nothing`"), "{got}");
    let mut missing = spec("duel", None);
    missing.roots = vec![PathBuf::from("no/such/packs")];
    assert!(first_error(missing).contains("cannot start the match"));
}

#[test]
fn an_unnamed_ruleset_must_be_the_only_one() {
    let got = first_error(spec("duel", None));
    assert!(got.contains("several rulesets"), "{got}");
}

fn cards_spec() -> LaunchSpec {
    let root = env!("CARGO_MANIFEST_DIR");
    LaunchSpec {
        roots: vec![PathBuf::from(root).join("../../fixtures/packs/cards")],
        scenario: "duel".into(),
        ruleset: Some("rs".into()),
        seed: 1,
        armies: vec![army("test_soldier"), army("test_soldier")],
        commands: 0,
    }
}

fn first_view(spec: LaunchSpec) -> PlayerView {
    let handle = launch(spec);
    match handle.recv_timeout(WAIT) {
        Ok(SimMessage::View(view)) => *view,
        other => panic!("expected a view, got {other:?}"),
    }
}

#[test]
fn a_card_shows_a_stat_as_the_aura_modifies_it() {
    let view = first_view(cards_spec());
    let card = view.cards.first().expect("a card per unit");
    assert_eq!(view.cards.len(), view.state.units.len());
    assert_eq!(card.name, "Test Soldier");
    assert_eq!(card.kind, "squad");
    let stat = |label: &str| card.stats.iter().find(|s| s.label == label).unwrap();
    assert_eq!((stat("ATK").base, stat("ATK").value), (3, 3));
    assert_eq!((stat("DEF").base, stat("DEF").value), (2, 3));
    let labels: Vec<&str> = card.stats.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels, ["ATK", "DEF"]);
    let alive: Vec<(bool, u32)> = card.figures.iter().map(|f| (f.alive, f.wounds)).collect();
    assert_eq!(alive, [(true, 0), (true, 0)]);
    assert_eq!(card.abilities[0].name, "Shieldwall");
    assert_eq!(card.abilities[0].text, "Adds 1 to defense.");
}

#[test]
fn cards_are_the_same_on_every_launch() {
    assert_eq!(
        first_view(cards_spec()).cards,
        first_view(cards_spec()).cards
    );
}

#[test]
fn a_view_carries_the_names_of_dice_and_markers() {
    let names = first_view(cards_spec()).names;
    let ids = || (0..64).map(strata_kernel::DefId);
    assert!(ids().any(|id| names.die(id) == Some("Combat")));
    assert!(ids().any(|id| names.marker(id) == Some("Wound")));
    assert!(!ids().any(|id| names.die(id) == Some("Wound")));
}

#[test]
fn a_card_stat_carries_the_trace_that_explains_it() {
    let view = first_view(cards_spec());
    let def = view.cards[0]
        .stats
        .iter()
        .find(|s| s.label == "DEF")
        .unwrap();
    let inspection = Inspection::of(def.trace.as_ref().expect("a queried stat has a trace"));
    assert_eq!(inspection.result, strata_kernel::Value::Int(def.value));
    assert_eq!(
        inspection.lines(),
        [
            "defense",
            "Base 2",
            "+1 from rs (Lua) = 3",
            "Clamped to 0..9",
            "Result 3"
        ]
    );
}
