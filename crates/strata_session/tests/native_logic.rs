//! Native descriptors written as plain data in a Lua module's registrations (#108). The fixture
//! is `fixtures/pack_test/native`.

use std::path::PathBuf;

use strata_session::pack_test::{PackTests, TestReport, summary};

fn run(filter: &str) -> Vec<TestReport> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pack_test/native");
    PackTests::load(&[root]).unwrap().run(Some(filter))
}

#[test]
fn a_registered_native_modifier_reads_the_terrain_from_the_map() {
    let reports = run("a registered native modifier");
    assert_eq!(reports.len(), 1);
    assert!(reports[0].outcome.is_ok(), "{}", summary(&reports));
}

#[test]
fn running_twice_gives_identical_reports() {
    assert_eq!(run("native"), run("native"));
}
