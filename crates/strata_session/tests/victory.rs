//! `core:victory`, through the `victory` fixture pack.

use std::path::PathBuf;

use strata_modules::native_modules;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/victory")];
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
    assert!(
        named(&reports, name).outcome.is_ok(),
        "{}",
        summary(&reports)
    );
}

#[test]
fn destroying_the_last_enemy_figure_wins() {
    passes("eliminate_all: destroying the last");
}

#[test]
fn a_figure_in_reserve_keeps_its_team_in_the_match() {
    passes("eliminate_all: a figure in reserve");
}

#[test]
fn no_command_is_taken_once_the_match_is_over() {
    let reports = run();
    let report = named(&reports, "refused: no command is taken");
    let failure = report.outcome.as_ref().unwrap_err();
    assert_eq!(failure.step, Some(4), "{failure:?}");
    assert!(failure.message.contains("the match is over"), "{failure:?}");
}

#[test]
fn holding_the_zone_at_enough_round_ends_wins() {
    passes("hold_zone: holding the zone");
}

#[test]
fn a_contested_zone_is_held_by_nobody() {
    passes("hold_zone: a zone both teams");
}

#[test]
fn the_round_limit_ends_the_match_by_points_remaining() {
    passes("round_limit: the end of the last round");
}

#[test]
fn a_tie_on_every_tiebreak_is_a_draw() {
    passes("round_limit: a tie on every tiebreak");
}

#[test]
fn a_later_tiebreak_settles_a_tie_on_an_earlier_one() {
    passes("round_limit: a later tiebreak");
}

#[test]
fn without_a_tiebreak_the_round_limit_is_a_draw() {
    passes("round_limit: without a tiebreak");
}

#[test]
fn the_first_listed_condition_wins_a_tie() {
    passes("the first listed condition wins a tie");
    passes("the first listed condition wins a tie, in either order");
}

#[test]
fn a_lua_function_is_a_victory_condition() {
    passes("a Lua function is a victory condition");
}

#[test]
fn every_test_passes_except_the_refused_one() {
    let reports = run();
    let failed: Vec<&str> = reports
        .iter()
        .filter(|r| r.outcome.is_err())
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(
        failed,
        ["refused: no command is taken once the match is over"]
    );
}

/// The same pack, run twice, commits the same state hash after every command.
#[test]
fn the_pack_is_deterministic() {
    let hashes = |reports: Vec<TestReport>| -> Vec<(String, Vec<u64>)> {
        let passed = reports
            .into_iter()
            .filter_map(|r| Some((r.name, r.outcome.ok()?)));
        passed.collect()
    };
    let (first, second) = (hashes(run()), hashes(run()));
    assert!(first.len() >= 10, "{first:?}");
    assert!(first.iter().all(|(_, h)| !h.is_empty()));
    assert_eq!(first, second);
}
