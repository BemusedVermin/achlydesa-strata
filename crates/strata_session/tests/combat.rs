//! `core:combat` through the `combat` fixture pack: the attack action, range, line of sight, the
//! height bonus, cover, a reaction in each window, `ctx.combat.attack`, the reactions the engine
//! refuses, an attacker destroyed mid-attack and modifiers that read the opponent from the
//! query's context.

use std::path::PathBuf;

use strata_kernel::{Sym, Value};
use strata_model::{Event, FigureId};
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

/// Whether the test must fail at a step the engine rejects.
fn refused(report: &TestReport) -> bool {
    report.name.starts_with("refused: ")
}

#[test]
fn every_combat_test_passes() {
    let reports = run();
    assert_eq!(reports.len(), 35, "{}", summary(&reports));
    let wrong = |r: &TestReport| r.outcome.is_ok() == refused(r);
    assert!(!reports.iter().any(wrong), "{}", summary(&reports));
}

#[test]
fn refused_steps_name_what_went_wrong() {
    let reports = run();
    let expected = [
        (
            "hits cannot be set before the attack roll",
            "`hits` cannot change in `combat.before_attack_roll`, only attack_dice, defense_dice",
        ),
        (
            "nothing can be set after the wounds",
            "nothing can change in `combat.after_wounds`",
        ),
        ("an attack cannot start during another", "issue #204"),
        (
            "a special attack cannot roll a negative pool",
            "ctx.combat.attack: `dice` must be 0..=1024, not -3",
        ),
        (
            "a preview cannot roll a negative pool",
            "ctx.combat.preview: `dice` must be 0..=1024, not -1",
        ),
    ];
    for (name, message) in expected {
        let name = format!("refused: {name}");
        let found = reports.iter().find(|r| r.name == name);
        let found = found.unwrap_or_else(|| panic!("no test `{name}`"));
        let failure = found.outcome.as_ref().expect_err(&name);
        assert!(failure.step.is_some(), "{found}");
        assert!(failure.message.contains("was rejected"), "{found}");
        assert!(failure.message.contains(message), "{found}");
    }
}

/// The field `key` of the only `attack` event of `report`.
fn attack_field(report: &TestReport, key: &str) -> Value {
    custom(report, "attack").get(key).cloned().unwrap()
}

#[test]
fn an_attacker_destroyed_before_its_roll_ends_the_attack() {
    let reports = run();
    let report = passed(
        &reports,
        "an attacker destroyed before its roll rolls nothing",
    );
    let rolled = report
        .events
        .iter()
        .any(|e| matches!(e, Event::DiceRolled { .. }));
    assert!(!rolled, "{:?}", report.events);
    assert_eq!(attack_field(report, "finished"), Value::Bool(false));
    assert_eq!(
        attack_field(report, "window"),
        Value::Str(Sym::new("combat.before_attack_roll"))
    );
    assert_eq!(attack_field(report, "hits"), Value::Int(0));
    assert_eq!(attack_field(report, "wounds"), Value::Int(0));
}

#[test]
fn an_attacker_destroyed_after_its_roll_wounds_nothing() {
    let reports = run();
    let report = passed(
        &reports,
        "an attacker destroyed after its roll wounds nothing",
    );
    let rolls = report.events.iter();
    let rolls = rolls.filter(|e| matches!(e, Event::DiceRolled { .. }));
    assert_eq!(
        rolls.count(),
        1,
        "only the attack roll: {:?}",
        report.events
    );
    assert_eq!(attack_field(report, "finished"), Value::Bool(false));
    assert_eq!(attack_field(report, "hits"), Value::Int(2));
    assert_eq!(attack_field(report, "blocks"), Value::Int(0));
    assert_eq!(attack_field(report, "wounds"), Value::Int(0));
    let wounded = report
        .events
        .iter()
        .any(|e| matches!(e, Event::WoundsApplied { figure, .. } if *figure == FigureId(1)));
    assert!(!wounded, "{:?}", report.events);
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
    assert_eq!(surveyed(report), vec![2, 3]);
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
    assert_eq!(attack.get("finished"), Ok(&Value::Bool(true)), "{attack:?}");
}

#[test]
fn the_attack_event_wounds_are_hits_minus_blocks() {
    for report in run().iter().filter(|r| r.outcome.is_ok()) {
        for event in &report.events {
            let Event::Custom { id, payload } = event else {
                continue;
            };
            if id.as_str() != "attack" {
                continue;
            }
            let field = |k: &str| payload.get(k).and_then(Value::as_int).unwrap();
            let expected = (field("hits") - field("blocks")).max(0);
            let finished = payload.get("finished") == Ok(&Value::Bool(true));
            let wounds = if finished { expected } else { 0 };
            assert_eq!(field("wounds"), wounds, "{}: {payload:?}", report.name);
        }
    }
}

#[test]
fn running_the_combat_tests_twice_gives_identical_hashes() {
    let (first, second) = (run(), run());
    assert_eq!(first, second);
    let unrigged = passed(&first, "an attack rolls from the command stream");
    assert!(!unrigged.outcome.as_ref().unwrap().is_empty());
}

/// The `attack_dice` and `defense_dice` of the only `peeked` event of `report`.
fn peeked_pools(report: &TestReport) -> (Value, Value) {
    let peeked = custom(report, "peeked");
    let field = |k: &str| peeked.get(k).cloned().unwrap();
    (field("attack_dice"), field("defense_dice"))
}

#[test]
fn combat_stat_queries_have_the_opponent_as_their_context() {
    let reports = run();
    let attacks = [
        (
            "an attack modifier reads the target from the context",
            (3, 2),
        ),
        (
            "the attack modifier skips a target that is not lower",
            (2, 3),
        ),
    ];
    for (name, pools) in attacks {
        let report = passed(&reports, name);
        let pool = |k| attack_field(report, k).as_int().unwrap();
        assert_eq!((pool("attack_dice"), pool("defense_dice")), pools, "{name}");
    }
    let low = passed(
        &reports,
        "a preview reads the lower target from the context",
    );
    assert_eq!(peeked_pools(low), (Value::Int(3), Value::Int(2)));
    let level = passed(
        &reports,
        "a preview reads the level target from the context",
    );
    assert_eq!(peeked_pools(level), (Value::Int(2), Value::Int(3)));
    let measure = passed(
        &reports,
        "ctx.query.stat takes a context as its third argument",
    );
    let measured = custom(measure, "measured");
    assert_eq!(measured.get("against"), Ok(&Value::Int(3)), "{measured:?}");
    assert_eq!(measured.get("alone"), Ok(&Value::Int(2)), "{measured:?}");
    let reach = passed(
        &reports,
        "a range modifier reads the target from the context",
    );
    assert_eq!(surveyed(reach), vec![1, 2]);
}

#[test]
fn context_modifiers_give_identical_hashes_twice() {
    let name = "a context modifier rolls from the command stream";
    let (first, second) = (run(), run());
    let (first, second) = (passed(&first, name), passed(&second, name));
    assert_eq!(first, second);
    let hashes = first.outcome.as_ref().unwrap();
    assert!(!hashes.is_empty());
    let attack = custom(first, "attack");
    assert_eq!(attack.get("attack_dice"), Ok(&Value::Int(3)), "{attack:?}");
}
