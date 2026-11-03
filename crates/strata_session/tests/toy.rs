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
const TESTS: [&str; 5] = [
    "units carry their stats",
    "the soldier attacks with its attack stat",
    "the archer attacks with its attack stat",
    "wounds accumulate over two attacks",
    "rigged dice decide the wounds",
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
fn pool_sizes_come_from_stats_and_the_tally_from_the_dice() {
    let reports = run();
    for ruleset in ["toy_rust", "toy_lua"] {
        let log = &report(&reports, ruleset, TESTS[1]).events;
        let pools: Vec<&Vec<u32>> = log
            .iter()
            .filter_map(|e| match e {
                Event::DiceRolled { results, .. } => Some(results),
                _ => None,
            })
            .collect();
        // The soldier attacks (3) and the archer defends (1).
        let sizes: Vec<usize> = pools.iter().map(|p| p.len()).collect();
        assert_eq!(sizes, [3, 1], "{ruleset}");
        let hits = pools[0].iter().filter(|r| **r <= 3).count();
        let blocks = pools[1].iter().filter(|r| (4..=5).contains(*r)).count();
        let tally = log.iter().find_map(|e| match e {
            Event::VarSet { value, .. } => value.as_int().ok(),
            _ => None,
        });
        assert_eq!(tally, Some(hits.saturating_sub(blocks) as i64), "{ruleset}");
    }
}

#[test]
fn running_the_toy_pack_twice_gives_identical_events_and_hashes() {
    let (first, second) = (run(), run());
    assert_eq!(first, second);
    let hashes = &report(&first, "toy_rust", TESTS[3]).outcome;
    assert_eq!(hashes.as_ref().unwrap().len(), 2);
}

#[test]
fn rigged_dice_decide_the_wounds_in_both_rulesets() {
    let reports = run();
    let what = "rigged dice decide the wounds";
    let (rust, lua) = (
        report(&reports, "toy_rust", what),
        report(&reports, "toy_lua", what),
    );
    assert!(
        rust.outcome.is_ok() && lua.outcome.is_ok(),
        "{}",
        summary(&reports)
    );
    assert_eq!(rust.events, lua.events);
    // Three hits against one block: the faces were forced, not drawn.
    let rolls: Vec<&Vec<u32>> = rust
        .events
        .iter()
        .filter_map(|e| match e {
            Event::DiceRolled { results, .. } => Some(results),
            _ => None,
        })
        .collect();
    assert_eq!(rolls, [&vec![1, 1, 1], &vec![4]]);
    let tally = rust.events.iter().find_map(|e| match e {
        Event::VarSet { value, .. } => value.as_int().ok(),
        _ => None,
    });
    assert_eq!(tally, Some(2));
}
