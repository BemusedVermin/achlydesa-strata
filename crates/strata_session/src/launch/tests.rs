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
