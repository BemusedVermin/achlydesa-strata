//! `core:deployment`, through the `deployment` fixture pack.

use std::path::PathBuf;

use strata_kernel::{Rotation, Space};
use strata_model::Event;
use strata_modules::native_modules;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/deployment")];
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
    let report = named(&reports, name);
    assert!(report.outcome.is_ok(), "{}", summary(&reports));
}

/// The test must fail at its only refused step, with `reason` in the message.
fn refused(name: &str, reason: &str) {
    let reports = run();
    let failure = named(&reports, name).outcome.as_ref().unwrap_err();
    assert!(
        failure.message.contains("`place_unit` was rejected"),
        "{failure:?}"
    );
    assert!(failure.message.contains(reason), "{failure:?}");
}

#[test]
fn a_unit_is_placed_on_the_surfaces_of_its_start_zone() {
    passes("a unit is placed");
}

#[test]
fn players_place_in_slot_order_and_the_last_placement_ends_deployment() {
    passes("players place in slot order");
}

#[test]
fn a_player_with_nothing_left_to_place_is_skipped() {
    passes("a player with nothing left");
}

#[test]
fn the_second_slot_cannot_place_first() {
    refused("the second slot", "not your turn");
}

#[test]
fn placing_outside_the_start_zone_is_refused() {
    refused("placing outside", "outside your start zone");
}

#[test]
fn placing_on_water_is_refused() {
    refused("placing on a surface", "cannot stand there");
}

#[test]
fn two_figures_cannot_share_a_space() {
    refused("two figures", "occupied");
}

#[test]
fn every_figure_needs_a_space() {
    refused("every figure", "has 2 figures");
}

#[test]
fn the_enemy_unit_cannot_be_placed() {
    refused("a player cannot place", "not your unit");
}

#[test]
fn an_occupied_space_is_refused() {
    refused("a space already", "occupied");
}

#[test]
fn a_two_space_figure_covers_both_its_cells() {
    passes("a two-space figure covers");
    let reports = run();
    let mut moved = named(&reports, "a two-space figure covers").events.iter();
    let cells = moved.find_map(|e| match e {
        Event::FigureMoved { cells, .. } => Some(cells.clone()),
        _ => None,
    });
    assert_eq!(cells, Some(vec![Space::new(2, 0, 1), Space::new(3, 0, 1)]));
}

#[test]
fn no_cell_of_a_body_may_overlap_another_figure() {
    refused("a rear cell cannot be placed on a figure", "occupied");
    refused("a figure cannot be placed on a rear cell", "occupied");
}

#[test]
fn every_cell_must_stand_inside_the_start_zone() {
    refused("a rear cell cannot stand on water", "cannot stand there");
    refused("a rear cell cannot leave", "outside your start zone");
}

/// The anchor, rotation and cells of the first figure moved in the test `name`.
fn first_move(reports: &[TestReport], name: &str) -> (Space, Rotation, Vec<Space>) {
    let mut events = named(reports, name).events.iter();
    let moved = events.find_map(|e| match e {
        Event::FigureMoved {
            to,
            rotation,
            cells,
            ..
        } => Some((*to, *rotation, cells.clone())),
        _ => None,
    });
    moved.unwrap_or_else(|| panic!("`{name}` moved no figure"))
}

#[test]
fn a_two_space_figure_deploys_at_every_rotation() {
    let reports = run();
    let anchor = Space::new(1, 0, 1);
    let rears = [(2, 0), (1, 1), (0, 1), (0, 0), (1, -1), (2, -1)];
    for (o, (q, r)) in (0u8..).zip(rears) {
        let name = format!("a cart deploys turned {o}");
        let report = named(&reports, &name);
        assert!(report.outcome.is_ok(), "{}", summary(&reports));
        let rotation = Rotation::new(o).unwrap();
        let cells = vec![anchor, Space::new(q, r, 1)];
        assert_eq!(first_move(&reports, &name), (anchor, rotation, cells));
    }
}

#[test]
fn a_missing_rotation_deploys_facing_the_default_way() {
    let reports = run();
    let (_, rotation, _) = first_move(&reports, "a two-space figure covers");
    assert_eq!(rotation, Rotation::default());
}

#[test]
fn a_turned_rear_cell_must_stay_in_the_start_zone() {
    refused("a turned rear cell cannot leave", "outside your start zone");
}

#[test]
fn a_turned_rear_cell_needs_a_surface_within_the_spread() {
    refused("a turned rear cell cannot hang", "cannot stand there");
    refused("a turned rear cell cannot spread", "cannot stand there");
}

#[test]
fn a_turned_rear_cell_cannot_overlap_another_figure() {
    refused(
        "a turned rear cell cannot be placed on a figure",
        "occupied",
    );
}

#[test]
fn a_rotation_past_five_is_refused() {
    refused("a rotation past 5", "a rotation is from 0 to 5");
}

#[test]
fn running_twice_gives_identical_reports_and_hashes() {
    assert_eq!(run(), run());
}
