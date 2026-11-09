use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use strata_kernel::{DefId, PackId, Sym, Value};
use strata_model::{Descriptor, Logic, ModifierDef, ModifierLayer, ScriptRef};

use super::*;

/// Native logic returns the descriptor's own value; counts calls.
#[derive(Default)]
struct Echo(AtomicUsize);

impl LogicEvaluator for Echo {
    fn eval(&self, d: &Descriptor, _: &EvalCtx<'_>) -> Result<Value, LogicError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(d.0.clone())
    }
}

/// Scripts return their `path` as an integer-or-bool lookup, or fail when unknown.
struct Table(BTreeMap<&'static str, Value>);

impl ScriptCaller for Table {
    fn call(&self, s: &ScriptRef, _: &EvalCtx<'_>) -> Result<Value, LogicError> {
        self.0
            .get(s.path.as_str())
            .cloned()
            .ok_or_else(|| LogicError(format!("no script {}", s.path)))
    }
}

fn native(v: Value) -> Logic {
    Logic::Native(Descriptor(v))
}

fn script(path: &str) -> Logic {
    Logic::Script(ScriptRef {
        pack: PackId::new("test"),
        def: DefId(1),
        path: Sym::new(path),
    })
}

fn modifier(
    query: &str,
    layer: ModifierLayer,
    source: &str,
    priority: i32,
    value: Logic,
) -> SourcedModifier {
    SourcedModifier {
        source: Sym::new(source),
        priority,
        def: ModifierDef {
            query: Sym::new(query),
            layer,
            select: None,
            when: None,
            value,
            overrides: false,
            on: Default::default(),
        },
    }
}

fn int(layer: ModifierLayer, source: &str, priority: i32, n: i64) -> SourcedModifier {
    modifier("defense", layer, source, priority, native(Value::Int(n)))
}

fn vote(source: &str, allow: bool, overrides: bool) -> SourcedModifier {
    let mut m = modifier(
        "can_enter",
        ModifierLayer::Set,
        source,
        0,
        native(Value::Bool(allow)),
    );
    m.def.overrides = overrides;
    m
}

fn bounds() -> BTreeMap<Sym, StatBounds> {
    BTreeMap::from([(Sym::new("defense"), StatBounds { min: 0, max: 10 })])
}

fn defense(base: i64) -> StatQuery {
    StatQuery {
        stat: Sym::new("defense"),
        base,
        subject: Value::Int(1),
        context: Value::Nil,
    }
}

fn run<T>(mods: &[SourcedModifier], f: impl FnOnce(&QueryEngine<'_>) -> T) -> T {
    let (echo, table, b) = (Echo::default(), Table(BTreeMap::new()), bounds());
    let logics = Logics {
        evaluator: &echo,
        scripts: &table,
    };
    f(&QueryEngine::new(mods, &b, logics))
}

fn enter(engine: &QueryEngine<'_>) -> bool {
    engine
        .query(&VoteQuery::can_enter(Value::Int(1), Value::Nil))
        .unwrap()
}

#[test]
fn layers_apply_set_add_mul_final() {
    let mods = [
        int(ModifierLayer::Final, "f", 0, 1),
        int(ModifierLayer::Mul, "m", 0, 2),
        int(ModifierLayer::Add, "a", 0, 1),
        int(ModifierLayer::Set, "s", 0, 3),
    ];
    // set 3, add 1 = 4, mul 2 = 8, final +1 = 9
    assert_eq!(run(&mods, |e| e.query(&defense(0)).unwrap()), 9);
}

#[test]
fn fractional_mul_truncates() {
    let half = Value::List(vec![Value::Int(1), Value::Int(2)]);
    let mods = [modifier(
        "defense",
        ModifierLayer::Mul,
        "m",
        0,
        native(half),
    )];
    assert_eq!(run(&mods, |e| e.query(&defense(5)).unwrap()), 2);
}

#[test]
fn order_never_depends_on_registration() {
    let mods = vec![
        int(ModifierLayer::Set, "b", 0, 2),
        int(ModifierLayer::Set, "a", 0, 7),
        int(ModifierLayer::Set, "c", 1, 4),
        int(ModifierLayer::Add, "z", 0, 1),
        int(ModifierLayer::Add, "y", 0, 1),
    ];
    let forward = run(&mods, |e| e.trace(&defense(0)).unwrap());
    let mut rev = mods.clone();
    rev.reverse();
    assert_eq!(forward, run(&rev, |e| e.trace(&defense(0)).unwrap()));
    // (Set, prio 0, "a") then (Set, prio 0, "b") then (Set, prio 1, "c") wins, then two adds.
    assert_eq!(forward.0, 6);
    let sources: Vec<&str> = forward.1.steps.iter().map(|s| s.source.as_str()).collect();
    assert_eq!(sources, ["a", "b", "c", "y", "z"]);
}

#[test]
fn identical_keys_break_ties_by_value() {
    let mods = vec![
        int(ModifierLayer::Set, "s", 0, 9),
        int(ModifierLayer::Set, "s", 0, 4),
    ];
    let mut rev = mods.clone();
    rev.reverse();
    let a = run(&mods, |e| e.query(&defense(0)).unwrap());
    assert_eq!(a, run(&rev, |e| e.query(&defense(0)).unwrap()));
    assert_eq!(a, 9);
}

#[test]
fn results_clamp_to_stat_bounds() {
    let up = [int(ModifierLayer::Add, "a", 0, 100)];
    assert_eq!(run(&up, |e| e.query(&defense(0)).unwrap()), 10);
    let down = [int(ModifierLayer::Add, "a", 0, -100)];
    let (value, trace) = run(&down, |e| e.trace(&defense(3)).unwrap());
    assert_eq!((value, trace.unclamped), (0, Some(-97)));
}

#[test]
fn unknown_stat_is_an_error() {
    let q = StatQuery {
        stat: Sym::new("nope"),
        ..defense(0)
    };
    assert!(matches!(
        run(&[], |e| e.query(&q)),
        Err(QueryError::UnknownStat(_))
    ));
}

#[test]
fn deny_beats_allow_beats_default() {
    assert!(run(&[], enter));
    assert!(run(&[vote("a", true, false)], enter));
    assert!(!run(
        &[vote("a", true, false), vote("b", false, false)],
        enter
    ));
    let engaged = VoteQuery::is_engaged(Value::Int(1), Value::Nil);
    assert!(!run(&[], |e| e.query(&engaged).unwrap()));
}

#[test]
fn override_beats_plain_votes() {
    let allow_over = [vote("a", false, false), vote("b", true, true)];
    assert!(run(&allow_over, enter));
    let both = [vote("a", true, true), vote("b", false, true)];
    assert!(!run(&both, enter));
}

#[test]
fn select_and_when_filter_modifiers() {
    let mut m = int(ModifierLayer::Add, "a", 0, 5);
    m.def.when = Some(native(Value::Bool(false)));
    let mut n = int(ModifierLayer::Add, "b", 0, 2);
    n.def.select = Some(native(Value::Bool(true)));
    assert_eq!(run(&[m, n], |e| e.query(&defense(0)).unwrap()), 2);
}

#[test]
fn script_logic_goes_to_the_caller() {
    let mods = [modifier(
        "defense",
        ModifierLayer::Add,
        "s",
        0,
        script("bonus"),
    )];
    let (echo, b) = (Echo::default(), bounds());
    let table = Table(BTreeMap::from([("bonus", Value::Int(3))]));
    let logics = Logics {
        evaluator: &echo,
        scripts: &table,
    };
    let engine = QueryEngine::new(&mods, &b, logics);
    assert_eq!(engine.query(&defense(1)).unwrap(), 4);
    assert_eq!(echo.0.load(Ordering::SeqCst), 0);

    let bad = [modifier(
        "defense",
        ModifierLayer::Add,
        "s",
        0,
        script("missing"),
    )];
    let engine = QueryEngine::new(&bad, &b, logics);
    assert!(matches!(
        engine.query(&defense(1)),
        Err(QueryError::Logic(_))
    ));
}

#[test]
fn wrong_value_type_names_the_source() {
    let mods = [modifier(
        "defense",
        ModifierLayer::Add,
        "bad",
        0,
        native(Value::Bool(true)),
    )];
    let err = run(&mods, |e| e.query(&defense(0))).unwrap_err();
    assert!(matches!(err, QueryError::Type { source_id, .. } if source_id.as_str() == "bad"));
}

#[test]
fn answers_are_memoized_until_invalidated() {
    let mods = [int(ModifierLayer::Add, "a", 0, 1)];
    let (echo, table, b) = (Echo::default(), Table(BTreeMap::new()), bounds());
    let logics = Logics {
        evaluator: &echo,
        scripts: &table,
    };
    let engine = QueryEngine::new(&mods, &b, logics);
    assert_eq!(engine.query(&defense(0)).unwrap(), 1);
    assert_eq!(engine.query(&defense(0)).unwrap(), 1);
    assert_eq!((echo.0.load(Ordering::SeqCst), engine.cached()), (1, 1));
    // A different base is a different question.
    engine.query(&defense(1)).unwrap();
    assert_eq!(engine.cached(), 2);
    // Traces bypass the cache.
    engine.trace(&defense(0)).unwrap();
    assert_eq!(echo.0.load(Ordering::SeqCst), 3);

    engine.invalidate();
    assert_eq!(engine.cached(), 0);
    engine.query(&defense(0)).unwrap();
    assert_eq!(echo.0.load(Ordering::SeqCst), 4);
}

#[test]
fn trace_records_inputs_steps_and_result() {
    let mods = [int(ModifierLayer::Add, "a", 2, 1)];
    let (value, trace) = run(&mods, |e| e.trace(&defense(4)).unwrap());
    assert_eq!(value, 5);
    assert_eq!(trace.base, Value::Int(4));
    assert_eq!(trace.subject, Value::Int(1));
    assert_eq!(trace.result, Value::Int(5));
    assert_eq!(trace.steps.len(), 1);
    assert_eq!(trace.steps[0].priority, 2);
}

#[test]
fn trace_marks_lua_steps_and_records_running_values_and_bounds() {
    let mods = [
        int(ModifierLayer::Add, "a", 0, 2),
        modifier("defense", ModifierLayer::Mul, "m", 0, script("double")),
    ];
    let (echo, b) = (Echo::default(), bounds());
    let table = Table(BTreeMap::from([("double", Value::Int(3))]));
    let logics = Logics {
        evaluator: &echo,
        scripts: &table,
    };
    let engine = QueryEngine::new(&mods, &b, logics);
    let (value, trace) = engine.trace(&defense(4)).unwrap();
    assert_eq!(value, 10);
    let steps: Vec<_> = trace.steps.iter().map(|s| (s.scripted, s.after)).collect();
    assert_eq!(steps, [(false, Some(6)), (true, Some(18))]);
    assert_eq!(trace.unclamped, Some(18));
    assert_eq!(trace.bounds, Some(StatBounds { min: 0, max: 10 }));
    let (_, votes) = run(&[vote("v", true, false)], |e| {
        e.trace(&VoteQuery::can_enter(Value::Int(1), Value::Nil))
            .unwrap()
    });
    assert_eq!((votes.steps[0].after, votes.bounds), (None, None));
}
