//! `core:engagement`, through the `engagement` fixture pack.

use std::path::PathBuf;

use strata_model::Event;
use strata_modules::native_modules;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn run() -> Vec<TestReport> {
    run_pack("engagement")
}

/// Runs every test of the fixture pack `pack`.
fn run_pack(pack: &str) -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs").join(pack)];
    let loaded = PackTests::load(&roots).unwrap();
    let tests = native_modules()
        .into_iter()
        .try_fold(loaded, PackTests::with_native);
    tests.unwrap().run(None)
}

/// The passing report of the test whose name starts with `name`.
fn passed(name: &str) -> TestReport {
    let reports = run();
    let found = reports.iter().find(|r| r.name.starts_with(name));
    let found = found.unwrap_or_else(|| panic!("no test `{name}`:\n{}", summary(&reports)));
    assert!(found.outcome.is_ok(), "{}", summary(&reports));
    found.clone()
}

/// How many dice rolls the test made.
fn rolls(report: &TestReport) -> usize {
    let rolled = report.events.iter();
    rolled
        .filter(|e| matches!(e, Event::DiceRolled { .. }))
        .count()
}

/// How many moves the test committed.
fn moves(report: &TestReport) -> usize {
    let moved = report.events.iter();
    moved
        .filter(|e| matches!(e, Event::FigureMoved { .. }))
        .count()
}

#[test]
fn leaving_an_enemy_on_the_same_level_rolls_one_reprisal() {
    let report = passed("leaving an adjacent enemy on the same level");
    assert_eq!(rolls(&report), 1);
}

#[test]
fn a_reprisal_hit_wounds_but_the_figure_still_moves() {
    let report = passed("a reprisal hit wounds");
    assert_eq!(moves(&report), 1);
}

#[test]
fn a_reprisal_that_destroys_the_figure_stops_its_move() {
    let report = passed("a reprisal that destroys");
    assert_eq!(moves(&report), 0);
}

#[test]
fn staying_next_to_the_enemy_provokes_nothing() {
    assert_eq!(rolls(&passed("moving to another space next to")), 0);
}

#[test]
fn leaving_an_ally_provokes_nothing() {
    assert_eq!(rolls(&passed("leaving an adjacent ally")), 0);
}

#[test]
fn an_enemy_whose_height_range_misses_is_not_engaged() {
    assert_eq!(rolls(&passed("an enemy a level up whose height")), 0);
}

#[test]
fn a_tall_figure_is_engaged_across_a_level() {
    assert_eq!(rolls(&passed("a tall figure is engaged")), 1);
}

#[test]
fn ignoring_heights_engages_across_a_level() {
    assert_eq!(rolls(&passed("with heights ignored")), 1);
}

#[test]
fn an_is_engaged_modifier_engages_across_a_level() {
    assert_eq!(rolls(&passed("an is_engaged modifier")), 1);
}

#[test]
fn a_figure_denied_reprisal_by_a_modifier_triggers_no_rolls() {
    let report = passed("a figure denied reprisal");
    assert_eq!(rolls(&report), 0);
    assert_eq!(moves(&report), 1);
}

#[test]
fn the_denial_does_not_cover_the_enemy() {
    assert_eq!(rolls(&passed("the denial is the figure's own")), 1);
}

#[test]
fn leaving_two_enemies_at_once_rolls_a_reprisal_for_each() {
    let report = passed("leaving two enemies at once");
    assert_eq!(rolls(&report), 2);
    assert_eq!(moves(&report), 1);
}

#[test]
fn a_reprisal_that_destroys_the_figure_skips_the_other_enemies() {
    let report = passed("when the first of two reprisals destroys");
    assert_eq!(rolls(&report), 1);
    assert_eq!(moves(&report), 0);
}

#[test]
fn staying_engaged_then_leaving_along_a_path_rolls_one_reprisal() {
    let report = passed("a path that stays next to the enemy");
    assert_eq!(rolls(&report), 1);
    assert_eq!(moves(&report), 1);
}

#[test]
fn leaving_the_same_enemy_twice_in_one_move_rolls_one_reprisal() {
    let report = passed("leaving the same enemy twice");
    assert_eq!(rolls(&report), 1);
    assert_eq!(moves(&report), 1);
}

/// The failure message of the misconfigured-pack test `name`.
fn misconfigured(name: &str) -> String {
    let reports = run_pack("engagement_misconfigured");
    let found = reports.iter().find(|r| r.name == name);
    let found = found.unwrap_or_else(|| panic!("no test `{name}`:\n{}", summary(&reports)));
    let failure = found
        .outcome
        .clone()
        .expect_err("a misconfigured reprisal must fail");
    assert_eq!(
        failure.step,
        Some(1),
        "the move fails, not the setup: {}",
        failure.message
    );
    assert_eq!(rolls(found), 0, "nothing is rolled before the check");
    failure.message
}

#[test]
fn a_misspelled_reprisal_die_names_the_config_key() {
    let message = misconfigured("misspelled_die");
    assert!(message.contains("core:engagement `die`"), "{message}");
    assert!(message.contains("unknown die `combt`"), "{message}");
}

#[test]
fn a_wound_face_the_die_lacks_names_the_config_key() {
    let message = misconfigured("misspelled_face");
    assert!(message.contains("core:engagement `wound_on`"), "{message}");
    assert!(
        message.contains("`hti` is not a face of die `combat`"),
        "{message}"
    );
}

#[test]
fn running_twice_gives_identical_reports_and_hashes() {
    let reports = run();
    assert_eq!(reports, run());
    assert!(
        reports.iter().all(|r| r.outcome.is_ok()),
        "{}",
        summary(&reports)
    );
}
