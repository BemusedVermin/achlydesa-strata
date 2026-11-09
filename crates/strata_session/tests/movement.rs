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

#[test]
fn running_the_movement_tests_twice_gives_identical_hashes() {
    assert_eq!(run(), run());
}
