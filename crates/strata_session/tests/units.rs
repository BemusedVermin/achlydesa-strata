//! `core:units` and the dice API, through the `units` fixture pack.

use std::path::PathBuf;

use strata_model::{Event, FigureId};
use strata_modules::native_modules;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/units")];
    let loaded = PackTests::load(&roots).unwrap();
    let tests = native_modules()
        .into_iter()
        .try_fold(loaded, PackTests::with_native)
        .unwrap();
    tests.run(None)
}

fn report<'a>(reports: &'a [TestReport], what: &str) -> &'a TestReport {
    reports.iter().find(|r| r.name == what).unwrap()
}

fn destroyed(report: &TestReport, figure: u32) -> usize {
    let hit = |e: &&Event| matches!(e, Event::FigureDestroyed(f) if *f == FigureId(figure));
    report.events.iter().filter(hit).count()
}

#[test]
fn every_pack_test_passes_except_the_one_that_expects_a_refusal() {
    let reports = run();
    let failed: Vec<&str> = reports
        .iter()
        .filter(|r| r.outcome.is_err())
        .map(|r| r.name.as_str())
        .collect();
    let refusal = "a destroyed figure cannot be wounded again";
    assert_eq!(failed, [refusal], "{}", summary(&reports));
    let message = &report(&reports, refusal)
        .outcome
        .as_ref()
        .unwrap_err()
        .message;
    assert!(message.contains("figure 0 is out of play"), "{message}");
}

#[test]
fn wounds_at_or_above_life_destroy_a_figure_exactly_once() {
    let reports = run();
    for what in [
        "wounds at life destroy the figure",
        "wounds above life destroy the figure",
    ] {
        assert_eq!(destroyed(report(&reports, what), 0), 1, "{what}");
    }
    // The refused second wound leaves no second event.
    let again = report(&reports, "a destroyed figure cannot be wounded again");
    assert_eq!(destroyed(again, 0), 1);
}

#[test]
fn wounds_below_life_destroy_nothing() {
    let reports = run();
    let below = report(&reports, "wounds below life leave the figure standing");
    assert!(below.outcome.is_ok());
    assert_eq!(destroyed(below, 0), 0);
}

#[test]
fn healing_removes_wounds_so_a_later_wound_does_not_destroy() {
    let reports = run();
    let healed = report(&reports, "healing takes wounds back off");
    assert!(healed.outcome.is_ok(), "{}", summary(&reports));
    assert_eq!(destroyed(healed, 0), 1, "only the final wound is lethal");
}

#[test]
fn a_squad_is_removed_when_its_last_figure_is_destroyed() {
    let reports = run();
    let squad = report(&reports, "a squad is removed with its last figure");
    assert!(squad.outcome.is_ok(), "{}", summary(&reports));
    let removed: Vec<&Event> = squad
        .events
        .iter()
        .filter(|e| matches!(e, Event::Custom { id, .. } if id.as_str() == "unit_removed"))
        .collect();
    assert_eq!(removed.len(), 1);
    // It follows the second destruction, not the first.
    let kinds: Vec<&str> = squad
        .events
        .iter()
        .filter_map(|e| match e {
            Event::FigureDestroyed(_) => Some("destroyed"),
            Event::Custom { id, .. } if id.as_str() == "unit_removed" => Some("removed"),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, ["destroyed", "destroyed", "removed"]);
}

#[test]
fn removing_a_figure_is_not_destroying_it() {
    let reports = run();
    let removed = report(
        &reports,
        "a removed figure leaves play without being destroyed",
    );
    assert!(removed.outcome.is_ok(), "{}", summary(&reports));
    assert_eq!(destroyed(removed, 1) + destroyed(removed, 2), 0);
    let gone = |e: &&Event| matches!(e, Event::FigureRemoved(_));
    assert_eq!(removed.events.iter().filter(gone).count(), 2);
}

#[test]
fn a_destruction_replaced_by_a_removal_announces_the_unit_once() {
    let reports = run();
    let exiled = report(
        &reports,
        "a destruction replaced by a removal removes the unit once",
    );
    assert!(exiled.outcome.is_ok(), "{}", summary(&reports));
    assert_eq!(destroyed(exiled, 1) + destroyed(exiled, 2), 0);
    let unit_removed =
        |e: &&Event| matches!(e, Event::Custom { id, .. } if id.as_str() == "unit_removed");
    assert_eq!(exiled.events.iter().filter(unit_removed).count(), 1);
}

fn hits(report: &TestReport) -> Option<i64> {
    report.events.iter().find_map(|e| match e {
        Event::VarSet { value, .. } => value.as_int().ok(),
        _ => None,
    })
}

fn rolls(report: &TestReport) -> Vec<Vec<u32>> {
    report
        .events
        .iter()
        .filter_map(|e| match e {
            Event::DiceRolled { results, .. } => Some(results.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_reroll_asks_then_replaces_one_die_with_a_new_roll() {
    let reports = run();
    let rerolled = report(&reports, "a reroll is a decision then a replacement roll");
    assert!(rerolled.outcome.is_ok(), "{}", summary(&reports));
    // The pool is the attack stat (2); the replacement is a single die.
    assert_eq!(rolls(rerolled), [vec![6, 6], vec![1]]);
    assert_eq!(hits(rerolled), Some(1));
}

#[test]
fn declining_a_reroll_rolls_no_replacement() {
    let reports = run();
    let kept = report(&reports, "declining a reroll keeps the first roll");
    assert!(kept.outcome.is_ok(), "{}", summary(&reports));
    assert_eq!(rolls(kept), [vec![1, 6]]);
    assert_eq!(hits(kept), Some(1));
}

#[test]
fn running_the_units_pack_twice_gives_identical_events_and_hashes() {
    assert_eq!(run(), run());
}
