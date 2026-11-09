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
    assert!(matches!(
        handle.recv_timeout(WAIT),
        Ok(SimMessage::Board(_))
    ));
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
    assert!(matches!(
        handle.recv_timeout(WAIT),
        Ok(SimMessage::Board(_))
    ));
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
fn the_board_arrives_first_and_matches_the_map() {
    let handle = launch(spec("duel", Some("toy_rust")));
    let Ok(SimMessage::Board(info)) = handle.recv_timeout(WAIT) else {
        panic!("expected the board first");
    };
    assert!(matches!(info.orientation.as_str(), "pointy" | "flat"));
    assert!(info.board.max_level().is_some());
    assert!(
        info.board
            .hexes()
            .any(|h| info.board.top_surface(h).is_some())
    );
}

#[test]
fn the_board_carries_the_packs_and_how_each_figure_is_drawn() {
    let handle = launch(spec("duel", Some("toy_rust")));
    let Ok(SimMessage::Board(info)) = handle.recv_timeout(WAIT) else {
        panic!("expected the board first");
    };
    let packs: Vec<_> = info.packs.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(packs, ["toy"]);
    assert!(info.packs[0].root.join("pack.lua").is_file());
    assert_eq!(
        info.looks.len(),
        2,
        "one look per figure entry of each unit"
    );
    for look in &info.looks {
        assert_eq!(look.pack.as_str(), "toy");
        assert_eq!(look.hit_zone, (0, 12));
        assert_eq!(look.footprint, [strata_kernel::Hex::ORIGIN]);
        assert_eq!((look.visual.as_ref(), look.color.as_ref()), (None, None));
    }
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
    assert_eq!(inspection.recompute(), Ok(def.value));
    let figure = view.cards[0].figures[0].figure.0;
    assert_eq!(
        inspection.lines(),
        [
            format!("defense of figure {figure}").as_str(),
            "Base 2",
            "+1 from rs (Lua) = 3",
            "Clamped to 0..9",
            "Result 3"
        ]
    );
}

/// The `command` fixture: player 0 has one soldier and player 1 none. The match allows the
/// commands from deployment up to one action.
fn command_spec() -> LaunchSpec {
    let root = env!("CARGO_MANIFEST_DIR");
    let mut army = army("test_soldier");
    if let Some(list) = &mut army {
        list.units[0].copies = 1;
    }
    LaunchSpec {
        roots: vec![PathBuf::from(root).join("../../fixtures/packs/command")],
        scenario: "duel".into(),
        ruleset: Some("rs".into()),
        seed: 5,
        armies: vec![army, None],
        commands: 6,
    }
}

fn custom(id: &str, payload: strata_kernel::Value) -> strata_model::Command {
    strata_model::Command::Custom {
        id: Sym::new(id),
        payload,
    }
}

/// The next view, skipping the events and board info that come before it.
fn view_after(handle: &SimHandle) -> PlayerView {
    loop {
        match handle.recv_timeout(WAIT) {
            Ok(SimMessage::View(view)) => return *view,
            Ok(SimMessage::Events(_) | SimMessage::Board(_)) => {}
            other => panic!("expected a view, got {other:?}"),
        }
    }
}

#[test]
fn a_real_match_offers_its_legal_commands_and_token_choice_to_the_viewer() {
    use strata_kernel::{DefId, Value};
    use strata_model::{Command, TokenPlacement, UnitId};

    let handle = launch(command_spec());
    let setup = view_after(&handle);
    assert_eq!(setup.legal, [], "nothing is listed before the round starts");
    assert_eq!(setup.tokens, None);
    let at = Value::List(vec![Value::List([0, 0, 1].map(Value::Int).to_vec())]);
    let place = [(Sym::new("unit"), Value::Int(0)), (Sym::new("at"), at)];
    handle.send_command(custom("place_unit", Value::Map(place.into())));
    let _ = view_after(&handle);
    handle.send_command(custom("start_round", Value::Nil));
    let placing = view_after(&handle);
    let choice = placing.tokens.expect("player 0 has a token to place");
    assert_eq!(choice.units, [UnitId(0)]);
    let put = choice.hand.iter().map(|token| TokenPlacement {
        unit: UnitId(0),
        token: *token,
    });
    handle.send_command(Command::PlaceTokens(put.collect()));
    let turn = view_after(&handle);
    assert_eq!(turn.tokens, None);
    assert_eq!(turn.legal, [Command::ActivateUnit(UnitId(0))]);
    handle.send_command(Command::ActivateUnit(UnitId(0)));
    let active = view_after(&handle);
    let walk = active.legal.iter().find_map(|c| match c {
        Command::Action { action, .. } => Some(*action),
        _ => None,
    });
    let walk = walk.expect("the soldier may walk");
    assert_eq!(active.names.action(walk), Some("walk"));
    assert!(active.legal.contains(&Command::EndActivation));
    assert_eq!(active.names.action(DefId(99)), None);
    assert_eq!(handle.shutdown(), Ok(()));
}
