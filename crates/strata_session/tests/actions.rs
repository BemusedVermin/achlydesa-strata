//! The built-in `action` command, through the `actions` fixture pack and `core:activation`.

use std::path::PathBuf;

use strata_model::Event;
use strata_modules::native_modules;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/actions")];
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

fn passes(reports: &[TestReport], what: &str) {
    let ran = report(reports, what);
    assert!(ran.outcome.is_ok(), "{}", summary(reports));
}

#[test]
fn a_squad_moves_every_figure_before_any_attacks() {
    let reports = run();
    let what = "a squad moves every figure before any attacks";
    passes(&reports, what);
    let spent = report(&reports, what)
        .events
        .iter()
        .filter(|e| matches!(e, Event::SlotsChanged { .. }))
        .count();
    assert_eq!(spent, 4);
}

#[test]
fn the_rules_refuse_actions_that_break_the_slots() {
    let reports = run();
    let early = report(
        &reports,
        "a figure cannot attack while another has yet to move",
    );
    assert!(message(early).contains("cannot spend those slots now"));
    let twice = report(&reports, "a figure cannot spend a slot twice");
    assert!(message(twice).contains("cannot spend those slots now"));
}

#[test]
fn targets_and_actors_are_checked_before_slots_are_spent() {
    let reports = run();
    let target = report(&reports, "an illegal target is refused");
    assert!(message(target).contains("illegal target"));
    let enemy = report(&reports, "a figure outside the active unit cannot act");
    assert!(message(enemy).contains("cannot act now"));
    let idle = report(&reports, "nothing can be done before a unit is activated");
    assert!(message(idle).contains("cannot act now"));
}

#[test]
fn one_figure_per_step_makes_each_figure_finish_before_the_next_starts() {
    let reports = run();
    passes(
        &reports,
        "with one figure per step, a figure finishes before the next starts",
    );
    let early = report(
        &reports,
        "with one figure per step, the next figure cannot move early",
    );
    assert!(message(early).contains("cannot spend those slots now"));
}

#[test]
fn an_activation_can_end_early_and_then_nothing_is_spent() {
    let reports = run();
    let ended = report(
        &reports,
        "an activation can end early and then nothing is spent",
    );
    assert!(message(ended).contains("cannot act now"));
    assert!(ended.events.contains(&Event::ActivationEnded));
}

#[test]
fn a_granted_move_reopens_the_move_step() {
    let reports = run();
    passes(&reports, "a granted move reopens the move step");
    let waits = report(&reports, "attacks wait for a granted move");
    assert!(message(waits).contains("cannot spend those slots now"));
    let outside = report(
        &reports,
        "a grant to a figure outside the activation is refused",
    );
    assert!(message(outside).contains("not in this activation"));
}

#[test]
fn running_the_pack_twice_gives_identical_events_and_hashes() {
    assert_eq!(run(), run());
}
