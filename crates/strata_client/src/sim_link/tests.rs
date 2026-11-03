use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use bevy::prelude::*;
use strata_kernel::{Sym, Value};
use strata_model::{ArmyEntry, ArmyList, Command, Text};
use strata_session::{LaunchSpec, launch};

use super::*;

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

fn spec(scenario: &str) -> LaunchSpec {
    let root = env!("CARGO_MANIFEST_DIR");
    LaunchSpec {
        roots: vec![PathBuf::from(root).join("../../fixtures/packs/toy")],
        scenario: scenario.into(),
        ruleset: Some("toy_rust".into()),
        seed: 7,
        armies: vec![army("test_soldier"), army("test_archer")],
        commands: 1,
    }
}

fn attack() -> Command {
    let payload = [
        (Sym::new("attacker"), Value::Int(0)),
        (Sym::new("target"), Value::Int(2)),
    ];
    Command::Custom {
        id: Sym::new("attack"),
        payload: Value::Map(payload.into()),
    }
}

fn app_for(spec: LaunchSpec) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(SimLinkPlugin::new(launch(spec)));
    app
}

/// Runs frames, as the real loop would, until `done` holds.
fn run_until(app: &mut App, done: impl Fn(&World) -> bool) {
    for _ in 0..2000 {
        app.update();
        if done(app.world()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the simulation never delivered");
}

#[test]
fn the_starting_view_is_drained_into_a_resource() {
    let mut app = app_for(spec("duel"));
    run_until(&mut app, |w| w.resource::<LatestView>().0.is_some());
    assert!(app.world().resource::<PendingEvents>().0.is_empty());
}

#[test]
fn a_command_sent_from_the_ecs_comes_back_as_a_pending_batch() {
    let mut app = app_for(spec("duel"));
    run_until(&mut app, |w| w.resource::<LatestView>().0.is_some());
    app.world_mut().write_message(SendCommand(attack()));
    run_until(&mut app, |w| {
        *w.resource::<SimStatus>() == SimStatus::Finished
    });
    let pending = app.world().resource::<PendingEvents>();
    assert_eq!(pending.0.len(), 1);
    assert!(!pending.0[0].is_empty());
    let view = app.world().resource::<LatestView>().0.as_ref();
    assert_eq!(view.unwrap().state.command_index, 1);
}

#[test]
fn a_match_that_fails_to_start_is_reported_in_the_status() {
    let mut app = app_for(spec("nowhere"));
    run_until(&mut app, |w| {
        matches!(w.resource::<SimStatus>(), SimStatus::Failed(_))
    });
}
