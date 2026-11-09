//! `core:deployment`, through the `deployment` fixture pack.

use std::path::PathBuf;

use strata_kernel::DefId;
use strata_kernel::Space;
use strata_kernel::Value;
use strata_model::Event;
use strata_modules::native_modules;
use strata_rules::event_kind;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/deployment")];
    let loaded = PackTests::load(&roots).unwrap();
    let tests = native_modules()
        .into_iter()
        .try_fold(loaded, PackTests::with_native);
    tests.unwrap().run(None)
}

fn named<'a>(reports: &'a [TestReport], name: &str) -> &'a TestReport {
    let found = reports.iter().find(|r| r.name.starts_with(name));
    found.unwrap_or_else(|| panic!("no test `{name}`:\n{}", summary(reports)))
}

fn passes(name: &str) {
    let reports = run();
    let report = named(&reports, name);
    assert!(report.outcome.is_ok(), "{}", summary(&reports));
}

/// The test must fail at its only refused step, with `reason` in the message.
fn refused(name: &str, reason: &str) {
    refused_command(name, "place_unit", reason);
}

/// The test must fail at its only refused step, a `command`, with `reason` in the message.
fn refused_command(name: &str, command: &str, reason: &str) {
    let reports = run();
    let failure = named(&reports, name).outcome.as_ref().unwrap_err();
    let rejected = format!("`{command}` was rejected");
    assert!(failure.message.contains(&rejected), "{failure:?}");
    assert!(failure.message.contains(reason), "{failure:?}");
}

#[test]
fn a_unit_is_placed_on_the_surfaces_of_its_start_zone() {
    passes("a unit is placed");
}

#[test]
fn players_place_in_slot_order_and_the_last_placement_ends_deployment() {
    passes("players place in slot order");
}

#[test]
fn a_player_with_nothing_left_to_place_is_skipped() {
    passes("a player with nothing left");
}

#[test]
fn the_second_slot_cannot_place_first() {
    refused("the second slot", "not your turn");
}

#[test]
fn placing_outside_the_start_zone_is_refused() {
    refused("placing outside", "outside your start zone");
}

#[test]
fn placing_on_water_is_refused() {
    refused("placing on a surface", "cannot stand there");
}

#[test]
fn two_figures_cannot_share_a_space() {
    refused("two figures", "occupied");
}

#[test]
fn every_figure_needs_a_space() {
    refused("every figure", "has 2 figures");
}

#[test]
fn the_enemy_unit_cannot_be_placed() {
    refused("a player cannot place", "not your unit");
}

#[test]
fn an_occupied_space_is_refused() {
    refused("a space already", "occupied");
}

#[test]
fn a_two_space_figure_covers_both_its_cells() {
    passes("a two-space figure covers");
    let reports = run();
    let mut moved = named(&reports, "a two-space figure covers").events.iter();
    let cells = moved.find_map(|e| match e {
        Event::FigureMoved { cells, .. } => Some(cells.clone()),
        _ => None,
    });
    assert_eq!(cells, Some(vec![Space::new(2, 0, 1), Space::new(3, 0, 1)]));
}

#[test]
fn no_cell_of_a_body_may_overlap_another_figure() {
    refused("a rear cell cannot be placed on a figure", "occupied");
    refused("a figure cannot be placed on a rear cell", "occupied");
}

#[test]
fn every_cell_must_stand_inside_the_start_zone() {
    refused("a rear cell cannot stand on water", "cannot stand there");
    refused("a rear cell cannot leave", "outside your start zone");
}

#[test]
fn running_twice_gives_identical_reports_and_hashes() {
    assert_eq!(run(), run());
}

#[test]
fn alternate_order_takes_one_unit_from_each_seat_in_turn() {
    passes("alternate order takes one unit");
    refused("in alternate order a seat cannot", "not your turn");
}

#[test]
fn army_order_places_a_whole_reserve_before_the_next_seat() {
    passes("army order places a whole reserve");
    refused("in army order the next seat waits", "not your turn");
}

/// The `flow` ruleset's phases in tree order: `setup` > `deploy`, then `round` >
/// (`initiative`, `turns`, `round_end`).
const SETUP: DefId = DefId(0);
const DEPLOY: DefId = DefId(1);
const ROUND: DefId = DefId(2);
const INITIATIVE: DefId = DefId(3);

#[test]
fn setup_and_deployment_lead_into_round_one() {
    passes("setup and deployment lead into round 1");
    let reports = run();
    let events = &named(&reports, "setup and deployment lead").events;
    let flow: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            Event::PhaseEntered(p) => Some(format!("enter {}", p.0)),
            Event::PhaseExited(p) => Some(format!("exit {}", p.0)),
            Event::RoundStarted(r) => Some(format!("round {r}")),
            Event::Custom { .. } | Event::TurnStarted { .. } => Some(event_kind(e).to_owned()),
            _ => None,
        })
        .collect();
    let at = |p: DefId| p.0;
    let want = [
        "placement_requested".into(),
        format!("enter {}", at(SETUP)),
        format!("enter {}", at(DEPLOY)),
        "placement_requested".into(),
        "placement_requested".into(),
        "deployment_done".into(),
        format!("exit {}", at(DEPLOY)),
        format!("exit {}", at(SETUP)),
        "setup_ended".into(),
        "round 1".into(),
        format!("enter {}", at(ROUND)),
        format!("enter {}", at(INITIATIVE)),
    ];
    assert_eq!(flow[..want.len()], want, "{flow:?}");
    assert!(
        flow[want.len()..].contains(&"turn_started".to_owned()),
        "{flow:?}"
    );
}

#[test]
fn a_round_cannot_start_before_deployment_and_round_one_starts_only_once() {
    refused_command(
        "a round cannot start before deployment",
        "start_round",
        "still waiting to deploy",
    );
    refused_command(
        "round 1 has already started",
        "start_round",
        "already under way",
    );
}

#[test]
fn a_round_cannot_start_while_setup_is_under_way() {
    // After one of two placements `setup` is open; the refusal names setup, not a round.
    refused_command(
        "a round cannot start after only some units",
        "start_round",
        "setup is not over",
    );
}

#[test]
fn a_deployment_done_listener_runs_after_the_rules_and_before_round_one() {
    // `passes` also pins that nobody was asked to order triggers: the test answers no decision.
    passes("a deployment_done listener runs");
    let reports = run();
    let events = &named(&reports, "a deployment_done listener runs").events;
    let kinds: Vec<&str> = events.iter().map(event_kind).collect();
    let done = kinds.iter().position(|k| *k == "deployment_done").unwrap();
    let want = [
        "deployment_done",
        "phase_exited",
        "phase_exited",
        "setup_ended",
        "heard",
        "round_started",
    ];
    assert_eq!(kinds[done..done + want.len()], want, "{kinds:?}");
    // The listener saw setup closed and round 1 not yet begun.
    let heard = events.iter().find_map(|e| match e {
        Event::Custom { id, payload } if id.as_str() == "heard" => Some(payload.clone()),
        _ => None,
    });
    let heard = heard.unwrap();
    assert_eq!(heard.get("round").unwrap(), &Value::Int(0));
    assert_eq!(heard.get("phases").unwrap(), &Value::Int(0));
}

#[test]
fn an_unknown_placement_order_fails_ruleset_resolution() {
    let reports = run();
    let report = named(&reports, "an unknown placement order");
    let failure = report.outcome.as_ref().unwrap_err();
    assert_eq!(failure.step, None, "{failure:?}");
    let want = "order must be `alternate` or `army`, not `whole`";
    assert!(failure.message.contains(want), "{failure:?}");
}
