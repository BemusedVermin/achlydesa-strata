//! `core:deployment`, through the `deployment` fixture pack.

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
fn running_twice_gives_identical_reports_and_hashes() {
    assert_eq!(run(), run());
}
