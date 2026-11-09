//! Frozen pack environments, through the `stateless` fixture pack: a `targets` script that
//! tries to keep state between calls is rejected the same way on every run (design §9.4).

use std::path::PathBuf;

use strata_modules::native_modules;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/stateless")];
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

#[test]
fn a_targets_script_with_only_locals_works() {
    let reports = run();
    let steady = report(&reports, "a targets script that keeps only locals works");
    assert!(steady.outcome.is_ok(), "{}", summary(&reports));
}

#[test]
fn a_targets_script_that_keeps_state_is_rejected() {
    let reports = run();
    let count = report(&reports, "a targets script that sets a global is rejected");
    assert!(
        message(count).contains("cannot set global `calls`"),
        "{}",
        message(count)
    );
    let remember = report(
        &reports,
        "a targets script that writes into a global table is rejected",
    );
    assert!(
        message(remember).contains("attempt to modify a frozen table"),
        "{}",
        message(remember)
    );
}

#[test]
fn running_the_pack_twice_gives_identical_reports_and_hashes() {
    assert_eq!(run(), run());
}
