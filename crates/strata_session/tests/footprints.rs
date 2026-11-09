//! Multi-space footprints and falling in `core:movement`, through the `footprints` fixture pack.

use std::path::PathBuf;

use strata_kernel::{Rotation, Space};
use strata_model::Event;
use strata_modules::native_modules;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/footprints")];
    let loaded = PackTests::load(&roots).unwrap();
    let tests = native_modules()
        .into_iter()
        .try_fold(loaded, PackTests::with_native);
    tests.unwrap().run(None)
}

fn report<'a>(reports: &'a [TestReport], what: &str) -> &'a TestReport {
    let found = reports.iter().find(|r| r.name == what);
    found.unwrap_or_else(|| panic!("no test `{what}`:\n{}", summary(reports)))
}

fn allowed(reports: &[TestReport], names: &[&str]) {
    for what in names {
        let ran = report(reports, what);
        assert!(ran.outcome.is_ok(), "{what}: {}", summary(reports));
    }
}

fn refused(reports: &[TestReport], names: &[&str]) {
    for what in names {
        let ran = report(reports, what);
        let message = &ran.outcome.as_ref().unwrap_err().message;
        assert!(message.contains("illegal target"), "{what}: {message}");
    }
}

/// The anchor, rotation and path of the last move the test made.
fn last_move(report: &TestReport) -> (Space, Rotation, Vec<Space>) {
    let mut moves = report.events.iter().filter_map(|e| match e {
        Event::FigureMoved {
            to, rotation, path, ..
        } => Some((*to, *rotation, path.clone())),
        _ => None,
    });
    moves.next_back().unwrap()
}

fn turned(steps: u8) -> Rotation {
    Rotation::new(steps).unwrap()
}

#[test]
fn a_snake_turns_and_reverses_as_its_rear_follows_the_leader() {
    let reports = run();
    allowed(
        &reports,
        &[
            "a snake turns as its rear follows the leader",
            "a snake reverses by stepping onto its rear",
        ],
    );
    refused(&reports, &["a snake cannot slide sideways"]);
    let turn = last_move(report(
        &reports,
        "a snake turns as its rear follows the leader",
    ));
    assert_eq!(
        turn,
        (Space::new(0, 1, 1), turned(4), vec![Space::new(0, 1, 1)])
    );
    let back = last_move(report(
        &reports,
        "a snake reverses by stepping onto its rear",
    ));
    assert_eq!(back.1, turned(3));
}

#[test]
fn a_rigid_shape_translates_without_turning() {
    let reports = run();
    allowed(&reports, &["a rigid shape translates"]);
    refused(&reports, &["a rigid shape cannot turn"]);
    let slid = last_move(report(&reports, "a rigid shape translates"));
    assert_eq!(slid.1, turned(0));
}

#[test]
fn no_cell_ends_over_empty_space_or_beyond_the_spread() {
    let reports = run();
    allowed(
        &reports,
        &[
            "a footprint can walk up to the edge",
            "a footprint may straddle a step within its spread",
        ],
    );
    refused(
        &reports,
        &[
            "a rear cell cannot end over empty space",
            "a footprint cannot straddle a step wider than its spread",
        ],
    );
}

#[test]
fn without_a_rotation_a_move_takes_the_cheapest() {
    let reports = run();
    let what = "without a rotation a move takes the cheapest one";
    allowed(&reports, &[what]);
    assert_eq!(last_move(report(&reports, what)).1, turned(4));
}

#[test]
fn falls_emit_figure_fell_and_wound_only_when_configured() {
    let reports = run();
    let names = [
        "a fall deals no damage by default",
        "a fall wounds per level beyond the limit when configured",
        "a fall can destroy a figure",
    ];
    allowed(&reports, &names);
    let wounds = |what: &str| {
        let found = report(&reports, what).events.iter();
        let amounts = found.filter_map(|e| match e {
            Event::WoundsApplied { amount, .. } => Some(*amount),
            _ => None,
        });
        amounts.collect::<Vec<_>>()
    };
    assert!(wounds(names[0]).is_empty());
    assert_eq!(
        wounds(names[1]),
        [1],
        "three levels down is one beyond `max_fall`"
    );
}

#[test]
fn every_footprint_test_is_checked_here() {
    assert_eq!(run().len(), 13, "{}", summary(&run()));
}

#[test]
fn running_the_footprint_tests_twice_gives_identical_hashes() {
    assert_eq!(run(), run());
}
