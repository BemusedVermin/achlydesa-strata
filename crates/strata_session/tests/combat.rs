//! `core:combat` through the `combat` fixture pack: the attack action, range, line of sight, the
//! height bonus, cover, a reaction in each window and `ctx.combat.attack`.

use std::path::PathBuf;

use strata_kernel::Value;
use strata_model::Event;
use strata_modules::native_modules;
use strata_session::pack_test::{PackTests, TestReport, summary};

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/combat")];
    let loaded = PackTests::load(&roots).unwrap();
    let tests = native_modules()
        .into_iter()
        .try_fold(loaded, PackTests::with_native);
    tests.unwrap().run(None)
}

/// The passing report of the test called `name`.
fn passed<'a>(reports: &'a [TestReport], name: &str) -> &'a TestReport {
    let found = reports.iter().find(|r| r.name == name);
    let found = found.unwrap_or_else(|| panic!("no test `{name}`:\n{}", summary(reports)));
    assert!(found.outcome.is_ok(), "{}", summary(reports));
    found
}

/// The payload of the only custom event called `id`.
fn custom<'a>(report: &'a TestReport, id: &str) -> &'a Value {
    let mut found = report.events.iter().filter_map(|e| match e {
        Event::Custom { id: kind, payload } if kind.as_str() == id => Some(payload),
        _ => None,
    });
    let first = found.next();
    let first = first.unwrap_or_else(|| panic!("{}: no `{id}` event", report.name));
    assert!(
        found.next().is_none(),
        "{}: several `{id}` events",
        report.name
    );
    first
}

/// The figures `survey` listed.
fn surveyed(report: &TestReport) -> Vec<i64> {
    let payload = custom(report, "surveyed");
    let count = payload.get("count").and_then(Value::as_int).unwrap();
    let targets = match payload.get("targets") {
        Ok(Value::List(items)) => items.iter().map(|v| v.as_int().unwrap()).collect(),
        _ => Vec::new(),
    };
    assert_eq!(targets.len() as i64, count, "{payload:?}");
    targets
}

#[test]
fn every_combat_test_passes() {
    let reports = run();
    assert_eq!(reports.len(), 21, "{}", summary(&reports));
    assert!(
        reports.iter().all(|r| r.outcome.is_ok()),
        "{}",
        summary(&reports)
    );
}

#[test]
fn an_archer_lists_only_the_enemies_within_its_range() {
    let reports = run();
    let report = passed(&reports, "an archer lists the enemies within its range");
    assert_eq!(surveyed(report), vec![1]);
}

#[test]
fn beyond_adjacency_an_enemy_out_of_sight_is_not_a_target() {
    let reports = run();
    let report = passed(
        &reports,
        "beyond adjacency only enemies in sight are listed",
    );
    assert_eq!(surveyed(report), vec![2]);
}

#[test]
fn a_denying_can_target_vote_leaves_no_target() {
    let reports = run();
    let report = passed(&reports, "a can_target vote can deny an adjacent target");
    assert_eq!(surveyed(report), Vec::<i64>::new());
}

#[test]
fn a_preview_shows_both_pools_and_the_cover_die() {
    let reports = run();
    let report = passed(&reports, "a preview shows the pools with cover");
    let peeked = custom(report, "peeked");
    let field = |k: &str| peeked.get(k).cloned().unwrap();
    assert_eq!(field("attack_dice"), Value::Int(2), "{peeked:?}");
    assert_eq!(field("defense_dice"), Value::Int(3), "{peeked:?}");
    assert_eq!(field("cover"), Value::Bool(true), "{peeked:?}");
    assert_eq!(field("ranged"), Value::Bool(true), "{peeked:?}");
}

#[test]
fn the_attack_event_carries_the_final_record() {
    let reports = run();
    let report = passed(&reports, "hits beyond blocks wound the target");
    let attack = custom(report, "attack");
    let field = |k: &str| attack.get(k).and_then(Value::as_int).unwrap();
    let got = [
        "attacker",
        "target",
        "attack_dice",
        "defense_dice",
        "hits",
        "blocks",
        "wounds",
    ];
    assert_eq!(got.map(field), [0, 1, 2, 2, 2, 1, 1], "{attack:?}");
}

#[test]
fn running_the_combat_tests_twice_gives_identical_hashes() {
    let (first, second) = (run(), run());
    assert_eq!(first, second);
    let unrigged = passed(&first, "an attack rolls from the command stream");
    assert!(!unrigged.outcome.as_ref().unwrap().is_empty());
}
