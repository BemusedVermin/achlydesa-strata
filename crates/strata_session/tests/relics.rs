//! `core:relics`, through the `relics` fixture pack.

use std::path::PathBuf;

use strata_model::Event;
use strata_modules::native_modules;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/relics")];
    let loaded = PackTests::load(&roots).unwrap();
    let tests = native_modules()
        .into_iter()
        .try_fold(loaded, PackTests::with_native);
    tests.unwrap().run(None)
}

fn passes(name: &str) {
    let reports = run();
    let found = reports.iter().find(|r| r.name.starts_with(name));
    let found = found.unwrap_or_else(|| panic!("no test `{name}`:\n{}", summary(&reports)));
    assert!(found.outcome.is_ok(), "{}", summary(&reports));
}

#[test]
fn ending_a_move_on_a_relic_claims_it() {
    passes("ending a move on a relic");
}

#[test]
fn the_bonus_disappears_exactly_when_the_occupant_leaves() {
    passes("the bonus disappears exactly");
}

#[test]
fn the_bonus_passes_to_the_next_occupant() {
    passes("the bonus passes");
}

#[test]
fn a_move_that_ends_elsewhere_claims_nothing() {
    passes("ending a move away");
}

#[test]
fn claiming_a_face_down_relic_reveals_it() {
    passes("claiming a face-down relic");
}

#[test]
fn an_on_claim_relic_runs_its_effect() {
    passes("an on-claim relic");
}

#[test]
fn removing_the_occupant_releases_the_relic_and_its_grants() {
    let reports = run();
    let name = "removing the occupant releases";
    let found = reports.iter().find(|r| r.name.starts_with(name));
    let found = found.unwrap_or_else(|| panic!("no test `{name}`:\n{}", summary(&reports)));
    assert!(found.outcome.is_ok(), "{}", summary(&reports));
    let released = |e: &&Event| matches!(e, Event::RelicReleased(_));
    assert_eq!(found.events.iter().filter(released).count(), 1);
    let detached = |e: &&Event| matches!(e, Event::AbilityDetached { .. });
    assert_eq!(found.events.iter().filter(detached).count(), 1);
}

#[test]
fn an_on_claim_effect_acts_for_the_figure_on_the_relic() {
    passes("an on-claim effect acts for the figure on the relic");
}

#[test]
fn a_rear_cell_claims_a_relic_and_releases_it_on_leaving() {
    passes("a rear cell claims a relic");
}

#[test]
fn one_figure_covering_two_spots_claims_both_relics() {
    passes("one figure covering two spots");
    let reports = run();
    let found = reports
        .iter()
        .find(|r| r.name.starts_with("one figure covering"));
    let claimed = found.unwrap().events.iter();
    let claimed = claimed.filter(|e| matches!(e, Event::RelicClaimed { .. }));
    assert_eq!(claimed.count(), 2);
}

#[test]
fn running_twice_gives_identical_reports_and_hashes() {
    assert_eq!(run(), run());
    let reports = run();
    assert!(
        reports.iter().all(|r| r.outcome.is_ok()),
        "{}",
        summary(&reports)
    );
}
