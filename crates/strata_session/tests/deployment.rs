//! `core:deployment` and the `act.deploy` step, through the `deployment` fixture pack.

use std::path::PathBuf;

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

fn report<'a>(reports: &'a [TestReport], name: &str) -> &'a TestReport {
    let found = reports.iter().find(|r| r.name == name);
    found.unwrap_or_else(|| panic!("no test `{name}`:\n{}", summary(reports)))
}

fn passes(name: &str) {
    let reports = run();
    let found = report(&reports, name);
    assert!(found.outcome.is_ok(), "{}", summary(&reports));
}

/// The test fails because its last `place_unit` was refused for a reason containing `why`.
fn refused(name: &str, why: &str) {
    let reports = run();
    let found = report(&reports, name);
    let failure = found
        .outcome
        .as_ref()
        .expect_err("the placement was accepted");
    assert!(
        failure.message.contains("`place_unit` was rejected") && failure.message.contains(why),
        "{name}: {}",
        failure.message
    );
}

#[test]
fn each_player_deploys_inside_their_start_zone_in_turn() {
    passes("each player deploys inside their start zone in turn");
}

#[test]
fn deploy_order_decides_who_places_first() {
    refused(
        "a player cannot deploy before an earlier slot has finished",
        "player 0 deploys first",
    );
    passes("the slot with the lowest deploy order deploys first");
}

#[test]
fn a_placement_outside_the_start_zone_is_refused() {
    refused(
        "a placement outside the start zone is refused",
        "outside start zone `west`",
    );
}

#[test]
fn a_placement_off_the_surfaces_is_refused() {
    refused(
        "a placement where the board has no terrain is refused",
        "is not a surface",
    );
    refused(
        "a placement at a level with no surface is refused",
        "is not a surface",
    );
    passes("a figure may deploy on top of a pillar in its zone");
}

#[test]
fn a_placement_on_another_figure_is_refused() {
    refused(
        "a placement on another figure is refused",
        "taken by another figure",
    );
    refused(
        "a two-space figure overlapping another figure is refused",
        "taken by another figure",
    );
}

#[test]
fn only_your_own_reserve_figures_can_be_deployed() {
    refused("a player cannot deploy an enemy unit", "is not yours");
    refused(
        "a unit with nothing in reserve cannot be deployed again",
        "no figure in reserve",
    );
}

#[test]
fn a_two_space_figure_needs_every_cell_in_the_zone_and_within_its_spread() {
    passes("a two-space figure deploys turned so both cells are in the zone");
    refused(
        "a two-space figure with a cell outside the zone is refused",
        "outside start zone `west`",
    );
    refused(
        "a two-space figure across too great a level difference is refused",
        "the cells span 2 levels, more than the 1 allowed",
    );
}

#[test]
fn the_start_zones_come_from_the_same_map_as_the_board() {
    passes("the start zone comes from the map the test names");
    refused(
        "the scenario map's start zone does not apply under another map",
        "outside start zone `west`",
    );
}

#[test]
fn running_twice_gives_identical_reports_and_hashes() {
    assert_eq!(run(), run());
}
