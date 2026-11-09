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
fn a_snake_moves_with_engagement_in_the_ruleset() {
    // Reachable spaces are flagged through `core:engagement`, asked about the body as a whole.
    let what = "a snake next to an enemy moves with engagement in the ruleset";
    allowed(&run(), &[what]);
}

#[test]
fn a_rigid_rear_cell_climbs_no_further_than_the_anchor_may() {
    refused(
        &run(),
        &["a rigid rear cell cannot climb further than the anchor may"],
    );
}

#[test]
fn a_rear_cell_without_headroom_falls_back_to_the_next_surface() {
    let reports = run();
    let what = "a rear cell without headroom stands on the next surface up";
    allowed(&reports, &[what]);
    let cells = report(&reports, what).events.iter().find_map(|e| match e {
        Event::FigureMoved { cells, .. } => Some(cells.clone()),
        _ => None,
    });
    assert_eq!(cells, Some(vec![Space::new(0, 0, 2), Space::new(1, 0, 3)]));
}

#[test]
fn a_rear_cell_is_occupied_for_moves_and_placements() {
    let reports = run();
    refused(
        &reports,
        &["a figure cannot stop on another figure's rear cell"],
    );
    let what = "a test cannot place a figure on another figure's rear cell";
    let failed = report(&reports, what).outcome.as_ref().unwrap_err();
    assert!(failed.message.contains("share space"), "{failed:?}");
}

#[test]
fn a_moved_body_records_every_cell() {
    let reports = run();
    let slid = report(&reports, "a rigid shape translates").events.iter();
    let cells = slid.filter_map(|e| match e {
        Event::FigureMoved { cells, .. } => Some(cells.clone()),
        _ => None,
    });
    let cells: Vec<_> = cells.collect();
    assert_eq!(cells, [vec![Space::new(0, 1, 1), Space::new(1, 1, 1)]]);
}

#[test]
fn every_footprint_test_is_checked_here() {
    assert_eq!(run().len(), 18, "{}", summary(&run()));
}

#[test]
fn running_the_footprint_tests_twice_gives_identical_hashes() {
    assert_eq!(run(), run());
}
