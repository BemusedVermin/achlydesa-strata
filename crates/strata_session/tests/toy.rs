//! The M2 gate: the toy pack's combat slot is Rust in one ruleset and Lua in the other, and the
//! same pack tests give the same results for both.

use std::path::PathBuf;

use strata_model::Event;
use strata_modules::native_modules;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/toy")];
    let loaded = PackTests::load(&roots).unwrap();
    let tests = native_modules()
        .into_iter()
        .try_fold(loaded, PackTests::with_native)
        .unwrap();
    tests.run(None)
}

/// The report of the test called `<ruleset>: <what>`.
fn report<'a>(reports: &'a [TestReport], ruleset: &str, what: &str) -> &'a TestReport {
    let name = format!("{ruleset}: {what}");
    reports.iter().find(|r| r.name == name).unwrap()
}

/// What each test checks, which both rulesets run.
const TESTS: [&str; 4] = [
    "units carry their stats",
    "an attack rolls both pools and tallies wounds",
    "an empty pool still rolls and records zero",
    "wounds accumulate over two attacks",
];

#[test]
fn every_pack_test_passes_for_both_rulesets() {
    let reports = run();
    assert_eq!(reports.len(), 2 * TESTS.len(), "{}", summary(&reports));
    assert!(
        reports.iter().all(|r| r.outcome.is_ok()),
        "{}",
        summary(&reports)
    );
}

#[test]
fn both_rulesets_commit_identical_event_logs() {
    let reports = run();
    for what in TESTS {
        let (rust, lua) = (
            report(&reports, "toy_rust", what),
            report(&reports, "toy_lua", what),
        );
        assert_eq!(rust.events, lua.events, "{what}");
    }
}

#[test]
fn the_event_logs_show_the_combat_slot_ran() {
    let reports = run();
    for ruleset in ["toy_rust", "toy_lua"] {
        let log = &report(&reports, ruleset, TESTS[1]).events;
        let rolled = log.iter().filter(|e| matches!(e, Event::DiceRolled { .. }));
        assert_eq!(rolled.count(), 2, "{ruleset}: one roll per pool");
        let wounded = log.iter().any(
            |e| matches!(e, Event::VarSet { value, .. } if value.as_int().is_ok_and(|n| n > 0)),
        );
        assert!(wounded, "{ruleset}: twelve dice against none must wound");
    }
}

#[test]
fn running_the_toy_pack_twice_gives_identical_events_and_hashes() {
    let (first, second) = (run(), run());
    assert_eq!(first, second);
    let hashes = &report(&first, "toy_rust", TESTS[3]).outcome;
    assert_eq!(hashes.as_ref().unwrap().len(), 2);
}
