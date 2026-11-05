//! `core:relics`, through the `relics` fixture pack.

use std::path::PathBuf;

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
fn running_twice_gives_identical_reports_and_hashes() {
    assert_eq!(run(), run());
    let reports = run();
    assert!(
        reports.iter().all(|r| r.outcome.is_ok()),
        "{}",
        summary(&reports)
    );
}
