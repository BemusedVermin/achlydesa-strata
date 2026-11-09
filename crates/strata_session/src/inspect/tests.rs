use std::collections::BTreeMap;

use strata_kernel::{DefId, PackId, Sym, Value};
use strata_model::{Descriptor, Logic, ModifierDef, ModifierLayer, ScriptRef};
use strata_rules::{
    EvalCtx, LogicError, LogicEvaluator, Logics, QueryEngine, ScriptCaller, SourcedModifier,
    StatBounds, StatQuery, VoteQuery,
};

use super::*;

/// Native logic returns the descriptor's own value.
struct Echo;

impl LogicEvaluator for Echo {
    fn eval(&self, d: &Descriptor, _: &EvalCtx<'_>) -> Result<Value, LogicError> {
        Ok(d.0.clone())
    }
}

/// A script returns the number in its path, standing in for a Lua function.
struct Scripts;

impl ScriptCaller for Scripts {
    fn call(&self, s: &ScriptRef, _: &EvalCtx<'_>) -> Result<Value, LogicError> {
        let n = s
            .path
            .as_str()
            .parse()
            .map_err(|_| LogicError("nan".into()))?;
        Ok(Value::Int(n))
    }
}

fn modifier(query: &str, layer: ModifierLayer, source: &str, value: Logic) -> SourcedModifier {
    SourcedModifier {
        source: Sym::new(source),
        priority: 0,
        def: ModifierDef {
            query: Sym::new(query),
            layer,
            select: None,
            when: None,
            value,
            overrides: false,
        },
    }
}

fn native(layer: ModifierLayer, source: &str, value: Value) -> SourcedModifier {
    modifier("defense", layer, source, Logic::Native(Descriptor(value)))
}

fn int(layer: ModifierLayer, source: &str, n: i64) -> SourcedModifier {
    native(layer, source, Value::Int(n))
}

fn lua(layer: ModifierLayer, source: &str, n: i64) -> SourcedModifier {
    let script = Logic::Script(ScriptRef {
        pack: PackId::new("test"),
        def: DefId(1),
        path: Sym::new(&n.to_string()),
    });
    modifier("defense", layer, source, script)
}

fn fraction(n: i64, d: i64) -> Value {
    Value::List(vec![Value::Int(n), Value::Int(d)])
}

/// The queried value and the inspection of the same query, with defense clamped to 0..99.
fn inspect(mods: &[SourcedModifier], base: i64) -> (i64, Inspection) {
    let bounds = BTreeMap::from([(Sym::new("defense"), StatBounds { min: 0, max: 99 })]);
    let logics = Logics {
        evaluator: &Echo,
        scripts: &Scripts,
    };
    let engine = QueryEngine::new(mods, &bounds, logics);
    let query = StatQuery {
        stat: Sym::new("defense"),
        base,
        subject: Value::Int(1),
        context: Value::Nil,
    };
    let value = engine.query(&query).unwrap();
    let (_, trace) = engine.trace(&query).unwrap();
    (value, Inspection::of(&trace))
}

#[test]
fn the_trace_arithmetic_reproduces_the_queried_value() {
    use ModifierLayer::{Add, Final, Mul, Set};
    let cases = [
        (vec![], 4),
        (vec![int(Add, "shieldwall", 1)], 4),
        (
            vec![int(Add, "a", 2), lua(Mul, "m", 3), int(Final, "f", -1)],
            4,
        ),
        (
            vec![int(Set, "s", 7), native(Mul, "half", fraction(1, 2))],
            4,
        ),
        (vec![lua(Set, "s", 50), int(Mul, "m", 3)], 4),
        (vec![int(Add, "curse", -9)], 4),
        (
            vec![native(Mul, "broken", fraction(3, 0)), int(Add, "a", 1)],
            4,
        ),
        (
            vec![native(Mul, "third", fraction(-1, 3)), int(Final, "f", 2)],
            10,
        ),
    ];
    for (mods, base) in cases {
        let (value, inspection) = inspect(&mods, base);
        assert_eq!(
            inspection.recompute(),
            Ok(value),
            "{:?}",
            inspection.lines()
        );
    }
}

#[test]
fn a_stat_trace_formats_base_modifiers_clamp_and_result() {
    use ModifierLayer::{Add, Final, Mul, Set};
    let mods = [
        int(Set, "rally", 60),
        int(Add, "shieldwall", 1),
        lua(Mul, "frenzy", 2),
        native(Mul, "half", fraction(1, 2)),
        int(Final, "wound", -1),
    ];
    let (value, inspection) = inspect(&mods, 4);
    assert_eq!(value, 60);
    assert_eq!(
        inspection.lines(),
        [
            "defense of figure 1",
            "Base 4",
            "Set 60 from rally (native) = 60",
            "+1 from shieldwall (native) = 61",
            "×2 from frenzy (Lua) = 122",
            "×1/2 from half (native) = 61",
            "Final -1 from wound (native) = 60",
            "Clamped to 0..99",
            "Result 60",
        ]
    );
    assert_eq!(inspection.lua_steps(), 1);
}

#[test]
fn a_clamp_that_bites_shows_the_unclamped_value() {
    let (value, inspection) = inspect(&[int(ModifierLayer::Add, "a", 200)], 4);
    assert_eq!(value, 99);
    let lines = inspection.lines();
    assert_eq!(
        lines[lines.len() - 2..],
        ["Clamped to 0..99 (was 204)", "Result 99"]
    );
}

#[test]
fn a_vote_trace_formats_votes_without_a_clamp() {
    let mut deny = modifier(
        "can_enter",
        ModifierLayer::Set,
        "wall",
        Logic::Native(Descriptor(Value::Bool(false))),
    );
    deny.def.overrides = true;
    let bounds = BTreeMap::new();
    let logics = Logics {
        evaluator: &Echo,
        scripts: &Scripts,
    };
    let engine = QueryEngine::new(std::slice::from_ref(&deny), &bounds, logics);
    let query = VoteQuery::can_enter(Value::Int(1), Value::Nil);
    let (_, trace) = engine.trace(&query).unwrap();
    let inspection = Inspection::of(&trace);
    assert_eq!(inspection.steps[0].change, Change::Vote(false));
    assert_eq!(
        inspection.lines(),
        [
            "can_enter of figure 1",
            "Base allow",
            "Deny from wall (native, override)",
            "Result deny",
        ]
    );
}

#[test]
fn a_zero_denominator_step_is_marked_ignored() {
    let mods = [native(ModifierLayer::Mul, "broken", fraction(3, 0))];
    let (value, inspection) = inspect(&mods, 4);
    assert_eq!(value, 4);
    assert_eq!(
        inspection.lines()[2],
        "×3/0 from broken (native, ignored: zero denominator) = 4"
    );
}

#[test]
fn recompute_reports_a_running_value_the_arithmetic_does_not_reach() {
    let (_, mut inspection) = inspect(&[int(ModifierLayer::Add, "a", 2)], 4);
    inspection.steps[0].after = Some(7);
    assert_eq!(
        inspection.recompute(),
        Err(TraceMismatch::Value {
            at: "the step from a".into(),
            recorded: Some(7),
            computed: 6,
        })
    );
    inspection.steps[0].change = Change::Vote(true);
    assert_eq!(inspection.recompute(), Err(TraceMismatch::NotAStat));
}

#[test]
fn the_heading_names_the_traced_figure() {
    let (_, mut inspection) = inspect(&[], 4);
    assert_eq!(inspection.heading(), "defense of figure 1");
    inspection.subject = Value::Nil;
    assert_eq!(inspection.heading(), "defense");
}

#[test]
fn an_ability_source_is_shown_by_its_display_name() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = root.join("../../fixtures/pack_test/grants");
    let tests = crate::pack_test::PackTests::load(&[root]).unwrap();
    let names = Names::of(&tests.registry, &[]);
    let mods = [
        int(ModifierLayer::Add, "t:test_veteran/abilities/1", 2),
        int(ModifierLayer::Add, "t:fury/grants/abilities/1", 1),
        int(ModifierLayer::Add, "t:sharp", 1),
        int(ModifierLayer::Add, "rs", 1),
    ];
    let (_, inspection) = inspect(&mods, 3);
    let named = inspection.named(&names);
    let shown: Vec<(&str, &str)> = named
        .steps
        .iter()
        .map(|s| (s.id.as_str(), s.source.as_str()))
        .collect();
    assert_eq!(
        shown,
        [
            ("rs", "rs"),
            ("t:fury/grants/abilities/1", "Fury"),
            ("t:sharp", "Sharp"),
            ("t:test_veteran/abilities/1", "Drill"),
        ]
    );
    assert!(
        named
            .lines()
            .contains(&"+2 from Drill (native) = 8".to_owned())
    );
    assert_eq!(names.ability("rs"), None);
}
