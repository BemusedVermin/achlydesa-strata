//! The `act.activate` and `act.end_activation` steps, through the `activation` fixture pack.

use std::path::PathBuf;

use strata_model::Event;
use strata_modules::native_modules;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/activation")];
    let loaded = PackTests::load(&roots).unwrap();
    let tests = native_modules()
        .into_iter()
        .try_fold(loaded, PackTests::with_native);
    tests.unwrap().run(None)
}

fn report<'a>(reports: &'a [TestReport], what: &str) -> &'a TestReport {
    reports.iter().find(|r| r.name == what).unwrap()
}

fn message(report: &TestReport) -> &str {
    &report.outcome.as_ref().unwrap_err().message
}

#[test]
fn a_player_activates_by_a_figure_of_the_unit_and_ends_it() {
    let reports = run();
    let ran = report(
        &reports,
        "a player activates a unit by one of its figures and ends it",
    );
    assert!(ran.outcome.is_ok(), "{}", summary(&reports));
    let started = ran
        .events
        .iter()
        .filter(|e| matches!(e, Event::ActivationStarted { .. }))
        .count();
    let ended = ran
        .events
        .iter()
        .filter(|e| matches!(e, Event::ActivationEnded))
        .count();
    assert_eq!((started, ended), (2, 1));
}

#[test]
fn the_rules_refuse_what_the_steps_submit() {
    let reports = run();
    let enemy = report(&reports, "a player cannot activate an enemy unit");
    assert!(message(enemy).contains("`activate_unit` was rejected"));
    assert!(message(enemy).contains("not your unit"));
    let idle = report(
        &reports,
        "ending an activation that has not started is refused",
    );
    assert!(message(idle).contains("`end_activation` was rejected"));
}

#[test]
fn an_unknown_figure_handle_fails_the_step() {
    let reports = run();
    let missing = report(
        &reports,
        "activating by an unknown figure handle fails the test",
    );
    assert!(message(missing).contains("p0.nobody#1"));
}
