use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::json;
use strata_kernel::{DefId, ModuleId, SlotId, Sym, Value};
use strata_model::{
    Answer, Command, Decision, Descriptor, DieKind, Event, FigureId, GameState, Logic, MatchSetup,
    PlayerId, ReactionDef, ReplacementDef, RulesetDef, SeedSource, TriggerDef,
};

use crate::*;

/// Emits `Custom(echoed, 7)` whatever set it off.
struct Echo;

impl NativeLogic for Echo {
    fn run(&self, ctx: &mut dyn Ctx, _: &dyn NativeEnv, _: &Event) -> Result<(), Halt> {
        ctx.emit(Event::Custom {
            id: Sym::new("echoed"),
            payload: Value::Int(7),
        })
    }
}

/// Turns a `go` command into a `go` event.
struct Go;

impl CommandHandler for Go {
    fn kind(&self) -> &str {
        "go"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, _: &Command) -> Result<(), Halt> {
        ctx.emit(Event::Custom {
            id: Sym::new("go"),
            payload: Value::Nil,
        })
    }
}

/// An add-on that registers the `go` command and a trigger on it that runs `effect`.
struct Addon {
    id: &'static str,
    effect: &'static str,
    provide: bool,
}

impl RuleModule for Addon {
    fn id(&self) -> ModuleId {
        ModuleId::new(self.id)
    }

    fn slot(&self) -> Option<SlotId> {
        None
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        static EMPTY: std::sync::OnceLock<ConfigSchema> = std::sync::OnceLock::new();
        EMPTY.get_or_init(ConfigSchema::default)
    }

    fn register(&self, r: &mut Registrar, _: &Value) -> Result<(), ModuleError> {
        r.command(Go);
        let effect = [(Sym::new("native"), Value::Str(Sym::new(self.effect)))];
        r.trigger(TriggerDef {
            event: Sym::new("go"),
            filter: None,
            limit: None,
            effect: Logic::Native(Descriptor(Value::Map(effect.into()))),
        });
        if self.provide {
            r.native("echo", Box::new(Echo));
        }
        Ok(())
    }
}

/// Answers nothing: every hook is Rust-native in these tests.
struct NoHooks;

impl HookRunner for NoHooks {
    fn replaces(&self, _: &ReplacementDef, _: &Event, _: &GameState) -> Result<bool, Halt> {
        Ok(false)
    }

    fn replace(&self, _: &ReplacementDef, e: &Event, _: &mut dyn Ctx) -> Result<Vec<Event>, Halt> {
        Ok(vec![e.clone()])
    }

    fn fires(&self, _: &TriggerDef, _: &Event, _: &GameState) -> Result<bool, Halt> {
        Ok(true)
    }

    fn fire(&self, _: &TriggerDef, _: &Event, _: &mut dyn Ctx) -> Result<(), Halt> {
        Err(Halt::reject(
            "a native trigger must not reach the hook runner",
        ))
    }

    fn reactor(&self, _: &ReactionDef, _: &GameState) -> Result<Option<PlayerId>, Halt> {
        Ok(None)
    }

    fn react(&self, _: &ReactionDef, _: &mut dyn Ctx) -> Result<(), Halt> {
        Ok(())
    }
}

fn runtime(modules: Vec<Addon>) -> Result<RulesRuntime, ResolveError> {
    let addons: Vec<_> = modules.iter().map(|m| json!({ "id": m.id })).collect();
    let body = json!({ "id": 0, "addons": addons });
    let sets: BTreeMap<Sym, RulesetDef> =
        [(Sym::new("rs"), serde_json::from_value(body).unwrap())].into();
    let mut catalog = ModuleCatalog::default();
    for module in modules {
        catalog.register(Box::new(module)).unwrap();
    }
    resolve("rs", &sets, &catalog)
}

fn go(rt: &RulesRuntime) -> (Vec<Event>, Vec<u64>) {
    let setup = MatchSetup {
        seed: SeedSource::Plain(1),
        ..Default::default()
    };
    let state = GameState {
        setup: Arc::new(setup),
        ..Default::default()
    };
    let mut engine = Engine::new(rt, &NoHooks, state);
    let command = Command::Custom {
        id: Sym::new("go"),
        payload: Value::Nil,
    };
    let Outcome::Committed(events) = engine.submit(PlayerId(0), command) else {
        panic!("the command should commit");
    };
    (events, engine.log().iter().map(|r| r.state_hash).collect())
}

fn echo() -> Addon {
    Addon {
        id: "t:echo",
        effect: "echo",
        provide: true,
    }
}

#[test]
fn a_trigger_runs_the_native_logic_its_effect_names() {
    let rt = runtime(vec![echo()]).unwrap();
    assert!(rt.native("echo").is_some());
    let (events, _) = go(&rt);
    let kinds: Vec<&str> = events.iter().map(event_kind).collect();
    assert_eq!(kinds, ["go", "echoed"]);
}

#[test]
fn running_native_logic_twice_gives_identical_hashes() {
    let rt = runtime(vec![echo()]).unwrap();
    assert_eq!(go(&rt), go(&rt));
}

#[test]
fn naming_native_logic_nobody_provides_is_a_resolve_error() {
    let missing = Addon {
        provide: false,
        ..echo()
    };
    let error = runtime(vec![missing]).err().unwrap();
    assert!(matches!(error, ResolveError::UnknownNative { native, .. } if native == "echo"));
}

#[test]
fn providing_the_same_native_name_twice_is_a_conflict() {
    let other = Addon {
        id: "t:other",
        ..echo()
    };
    let error = runtime(vec![echo(), other]).err().unwrap();
    assert!(matches!(error, ResolveError::Conflict { what, .. } if what.contains("echo")));
}

#[test]
fn only_a_lone_native_descriptor_names_native_logic() {
    let map = |pairs: &[(&str, &str)]| {
        let entries = pairs
            .iter()
            .map(|(k, v)| (Sym::new(k), Value::Str(Sym::new(v))));
        Logic::Native(Descriptor(Value::Map(entries.collect())))
    };
    assert_eq!(native_name(&map(&[("native", "echo")])), Some("echo"));
    assert_eq!(native_name(&map(&[("sel", "self")])), None);
    assert_eq!(native_name(&Logic::Native(Descriptor(Value::Int(1)))), None);
}

/// A fixed environment: one die `d` with faces `a a b`, and every stat is 4.
struct Fixed;

impl NativeEnv for Fixed {
    fn stat(&self, _: &GameState, _: FigureId, stat: &str) -> Result<i64, LogicError> {
        (stat == "life")
            .then_some(4)
            .ok_or_else(|| LogicError(format!("unknown stat `{stat}`")))
    }

    fn die(&self, name: &str) -> Option<(DefId, DieKind)> {
        let faces = ["a", "a", "b"].map(Sym::new).to_vec();
        (name == "d").then_some((DefId(5), DieKind::Faces(faces)))
    }
}

/// A `Ctx` that applies what is emitted, with a fixed seed.
struct Bench {
    state: GameState,
    rng: strata_kernel::CommandRng,
    events: Vec<Event>,
}

impl Bench {
    fn new() -> Self {
        Self {
            state: GameState::default(),
            rng: strata_kernel::CommandRng::for_command(3, 0),
            events: Vec::new(),
        }
    }
}

impl Ctx for Bench {
    fn state(&self) -> &GameState {
        &self.state
    }

    fn rng(&mut self) -> &mut strata_kernel::CommandRng {
        &mut self.rng
    }

    fn emit(&mut self, e: Event) -> Result<(), Halt> {
        self.events.push(e);
        Ok(())
    }

    fn window(&mut self, _: &strata_kernel::WindowId) -> Result<(), Halt> {
        Ok(())
    }

    fn choose(&mut self, _: PlayerId, _: Decision) -> Result<Answer, Halt> {
        Err(Halt::reject("the bench asks nothing"))
    }
}

#[test]
fn a_pool_rolls_the_named_die_and_reports_its_faces() {
    let mut bench = Bench::new();
    let roll = roll_pool(&mut bench, &Fixed, "d", 40).unwrap();
    assert_eq!(roll.results.len(), 40);
    assert!(roll.results.iter().all(|r| (1..=3).contains(r)));
    let (a, b) = (roll.count("a"), roll.count("b"));
    assert_eq!((a + b, roll.count("c")), (40, 0));
    assert_eq!(b, roll.results.iter().filter(|r| **r == 3).count());
    let rolled = Event::DiceRolled {
        die: DefId(5),
        results: roll.results.clone(),
    };
    assert_eq!(bench.events, [rolled]);
}

#[test]
fn a_pool_repeats_for_the_same_seed() {
    let first = roll_pool(&mut Bench::new(), &Fixed, "d", 16).unwrap();
    assert_eq!(
        first,
        roll_pool(&mut Bench::new(), &Fixed, "d", 16).unwrap()
    );
}

#[test]
fn a_pool_rejects_an_unknown_die_and_a_bad_size() {
    let mut bench = Bench::new();
    assert!(roll_pool(&mut bench, &Fixed, "nope", 1).is_err());
    assert!(roll_pool(&mut bench, &Fixed, "d", -1).is_err());
    assert!(roll_pool(&mut bench, &Fixed, "d", MAX_POOL + 1).is_err());
    assert!(
        roll_pool(&mut bench, &Fixed, "d", 0)
            .unwrap()
            .results
            .is_empty()
    );
    assert!(roll_pool(&mut bench, &NoEnv, "d", 1).is_err());
}

#[test]
fn the_environment_answers_stat_queries_and_a_missing_one_fails() {
    let state = GameState::default();
    assert_eq!(Fixed.stat(&state, FigureId(0), "life"), Ok(4));
    assert!(Fixed.stat(&state, FigureId(0), "move").is_err());
    assert!(NoEnv.stat(&state, FigureId(0), "life").is_err());
}

/// `Fixed`, with the next roll forced to `forced`.
struct Rigged(Result<Vec<u32>, String>);

impl NativeEnv for Rigged {
    fn stat(&self, state: &GameState, figure: FigureId, stat: &str) -> Result<i64, LogicError> {
        Fixed.stat(state, figure, stat)
    }

    fn die(&self, name: &str) -> Option<(DefId, DieKind)> {
        Fixed.die(name)
    }

    fn forced_roll(&self, _: &str, _: usize) -> Option<Result<Vec<u32>, String>> {
        Some(self.0.clone())
    }
}

#[test]
fn a_forced_roll_replaces_the_results_but_not_the_stream() {
    let mut rigged = Bench::new();
    let roll = roll_pool(&mut rigged, &Rigged(Ok(vec![3, 3])), "d", 2).unwrap();
    assert_eq!((roll.results.clone(), roll.count("b")), (vec![3, 3], 2));
    assert!(
        matches!(&rigged.events[..], [Event::DiceRolled { results, .. }] if *results == [3, 3])
    );
    let mut plain = Bench::new();
    roll_pool(&mut plain, &Fixed, "d", 2).unwrap();
    // Both streams advanced by two draws, so the next roll agrees.
    let after = |bench: &mut Bench| roll_pool(bench, &Fixed, "d", 5).unwrap();
    assert_eq!(after(&mut rigged), after(&mut plain));
}

#[test]
fn a_bad_forced_roll_is_rejected() {
    let bad = Rigged(Err("the rig has no faces left".into()));
    let error = roll_pool(&mut Bench::new(), &bad, "d", 1).unwrap_err();
    assert!(matches!(error, Halt::Rejected(v) if v.0.contains("no faces left")));
}
