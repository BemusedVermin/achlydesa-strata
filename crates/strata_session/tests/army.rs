//! `core:army`, through the `army` fixture pack.

use std::path::PathBuf;

use strata_modules::native_modules;
use strata_rules::RuleOrigin;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn load() -> PackTests {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/army")];
    let loaded = PackTests::load(&roots).unwrap();
    let tests = native_modules()
        .into_iter()
        .try_fold(loaded, PackTests::with_native);
    tests.unwrap()
}

fn passes(name: &str) {
    let reports: Vec<TestReport> = load().run(None);
    let found = reports.iter().find(|r| r.name.starts_with(name));
    let report = found.unwrap_or_else(|| panic!("no test `{name}`:\n{}", summary(&reports)));
    assert!(report.outcome.is_ok(), "{}", summary(&reports));
}

#[test]
fn a_list_within_every_rule_is_legal() {
    passes("a list within every rule");
}

#[test]
fn max_points_and_max_copies_catch_an_expensive_list() {
    passes("max_points and max_copies");
}

#[test]
fn a_unique_unit_is_taken_once() {
    passes("a unique unit");
}

#[test]
fn a_scenarios_army_rules_and_its_formats_rules_both_apply() {
    passes("a scenario's army rules and its format's");
}

#[test]
fn a_scenario_alone_applies_its_own_rules() {
    passes("a scenario alone");
}

#[test]
fn a_format_without_rules_allows_any_list() {
    passes("a format without rules");
}

#[test]
fn a_lua_function_is_an_army_rule() {
    passes("a Lua function is an army rule");
    passes("a Lua army rule that returns nil");
}

#[test]
fn every_test_in_the_pack_passes() {
    let reports = load().run(None);
    assert!(
        reports.iter().all(|r| r.outcome.is_ok()),
        "{}",
        summary(&reports)
    );
}

#[test]
fn each_violation_names_its_rule_origin_and_says_what_is_wrong() {
    let found = load().validate_army("rs", "army:mixed", Some("duel_200"), Some("strict"));
    let found = found.unwrap();
    let origins: Vec<(RuleOrigin, usize)> = found.iter().map(|v| (v.origin, v.index)).collect();
    assert_eq!(
        origins,
        [
            (RuleOrigin::Format, 0),
            (RuleOrigin::Scenario, 0),
            (RuleOrigin::Scenario, 1),
            (RuleOrigin::Scenario, 2),
        ]
    );
    assert_eq!(
        found[0].message,
        "the army costs 210 points, over the limit of 200"
    );
    assert_eq!(found[1].message, "`army:test_archer` is banned");
    assert_eq!(
        found[3].message,
        "the army covers 7 spaces, over the limit of 6"
    );
}

#[test]
fn a_ruleset_without_an_army_module_cannot_validate() {
    let tests = PackTests::load(&[
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/packs/army")
    ])
    .unwrap();
    let found = tests.validate_army("rs", "legal", Some("duel_200"), None);
    assert!(found.is_err(), "{found:?}");
}
