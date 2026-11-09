//! The ordinary move action, through the `movement` fixture pack, `core:movement` and
//! `core:activation`.

use std::path::PathBuf;

use strata_kernel::Space;
use strata_model::Event;
use strata_modules::native_modules;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/pack_test/movement")];
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

fn moved(report: &TestReport) -> Vec<(Space, Vec<Space>)> {
    let moves = report.events.iter().filter_map(|e| match e {
        Event::FigureMoved { to, path, .. } => Some((*to, path.clone())),
        _ => None,
    });
    moves.collect()
}

#[test]
fn a_figure_walks_to_a_reachable_space_and_the_event_carries_the_path() {
    let reports = run();
    let far = report(&reports, "a figure walks as far as its move allows");
    assert!(far.outcome.is_ok(), "{}", summary(&reports));
    let path: Vec<_> = (1..=3).map(|q| Space::new(q, 0, 1)).collect();
    let walked = moved(far);
    assert_eq!(walked.last(), Some(&(Space::new(3, 0, 1), path)));
}

#[test]
fn unreachable_spaces_are_illegal_targets() {
    let reports = run();
    for what in [
        "a figure cannot walk past its move",
        "a figure cannot walk through stop-on-enter terrain",
        "a figure cannot stop on an occupied space",
    ] {
        assert!(message(report(&reports, what)).contains("illegal target"));
    }
}

#[test]
fn stop_on_enter_terrain_and_other_figures_shape_the_walk() {
    let reports = run();
    for what in [
        "a figure can enter stop-on-enter terrain",
        "a figure can walk through an occupied space",
    ] {
        assert!(
            report(&reports, what).outcome.is_ok(),
            "{}",
            summary(&reports)
        );
    }
}

fn allowed(reports: &[TestReport], names: &[&str]) {
    for what in names {
        let ran = report(reports, what);
        assert!(ran.outcome.is_ok(), "{what}: {}", summary(reports));
    }
}

fn refused(reports: &[TestReport], names: &[&str]) {
    for what in names {
        assert!(message(report(reports, what)).contains("illegal target"));
    }
}

#[test]
fn climbing_is_limited_by_the_config_and_by_can_climb() {
    let reports = run();
    allowed(
        &reports,
        &[
            "a figure climbs one level",
            "a wider climb limit lets a figure climb two levels",
        ],
    );
    refused(
        &reports,
        &[
            "a figure cannot climb two levels by default",
            "a can_climb veto stops a climb",
        ],
    );
}

#[test]
fn bridges_can_be_walked_under_climbed_onto_and_fallen_from() {
    let reports = run();
    allowed(
        &reports,
        &[
            "a short figure walks under a bridge",
            "headroom can be ignored",
            "a figure with a long climb can reach a bridge deck",
            "a figure can step off a bridge and fall",
        ],
    );
    refused(
        &reports,
        &[
            "a tall figure cannot walk under a bridge",
            "a figure cannot climb onto a bridge deck by default",
            "a fall ends the move",
        ],
    );
    let under = moved(report(&reports, "a short figure walks under a bridge"));
    let spaces: Vec<_> = (1..=3).map(|q| Space::new(q, 0, 1)).collect();
    assert_eq!(under.last(), Some(&(Space::new(3, 0, 1), spaces)));
    let deck = "a figure with a long climb can reach a bridge deck";
    let over = moved(report(&reports, deck));
    assert_eq!(over.last().map(|m| m.0), Some(Space::new(2, 0, 4)));
}

#[test]
fn move_cost_and_can_enter_modifiers_change_where_a_figure_can_go() {
    let reports = run();
    allowed(
        &reports,
        &[
            "a move_cost modifier makes steps dearer",
            "a can_enter veto leaves other spaces open",
        ],
    );
    refused(
        &reports,
        &[
            "a dearer step shortens the move",
            "a can_enter veto closes a space",
        ],
    );
}

#[test]
fn every_movement_test_is_checked_here() {
    assert_eq!(run().len(), 21);
}

#[test]
fn running_the_movement_tests_twice_gives_identical_hashes() {
    assert_eq!(run(), run());
}
