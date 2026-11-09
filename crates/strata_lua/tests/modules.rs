use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use strata_kernel::{CommandRng, DefId, ModuleId, PackId, Sym, Value};
use strata_lua::{
    DataStage, FrozenRecord, Limits, LuaModule, LuaRules, ModuleBinding, PackSource, RuleHost,
    RuntimeLua, freeze,
};
use strata_model::{
    Answer, Command, Decision, Descriptor, DieKind, Event, FigureId, FigureState, GameState, Logic,
    MatchSetup, ModuleRef, PlayerId, PlayerState, RulesetDef, SeedSource, VarKey,
};
use strata_rules::{
    ActionRunner, CommandHandler, ConfigSchema, Ctx, Engine, Halt, LogicError, LogicEvaluator,
    Logics, LuaApi, ModuleCatalog, ModuleError, Outcome, QueryEngine, Registrar, RuleModule,
    RulesRuntime, SourcedModifier, StatBounds, StatQuery, resolve,
};

const DATA: &str = r#"
rule_module {
  id = 'defender_first',
  config = { min_wounds = { type = 'int', default = 0 } },
  register = function(r, cfg)
    r.action {
      id = 'attack',
      slots = { 'attack' },
      targets = function(ctx, attacker) return ctx.select({ sel = 'enemies' }, attacker) end,
      resolve = function(ctx, attacker, target)
        local def = ctx.dice.roll('combat', ctx.query.stat(target, 'defense'))
        local atk = ctx.dice.roll('combat', 3)
        local wounds = math.max(cfg.min_wounds, atk:count('hit') - def:count('block'))
        ctx.units.wound(target, wounds)
      end,
    }
    r.modifier {
      query = 'defense',
      layer = 'add',
      value = function() return cfg.min_wounds + 1 end,
    }
    r.trigger {
      event = 'wounds_applied',
      effect = function(ctx, event)
        local vars = ctx.vars('t:defender_first')
        vars.hits = (vars.hits or 0) + event.WoundsApplied.amount
      end,
    }
    r.window('defender_first.after_roll')
  end,
}

rule_module {
  id = 'probe',
  register = function(r)
    r.action {
      id = 'probe',
      slots = {},
      resolve = function(ctx, figure)
        local writable = pcall(function() ctx.state.round = 9 end)
        local deep = pcall(function() ctx.state.figures[1].status = 'destroyed' end)
        ctx.emit('state', { writable = writable or deep, round = ctx.state.round,
                            figures = #ctx.state.figures, active = ctx.state.active })
        ctx.emit('select', ctx.select({ sel = 'allies' }))
        ctx.emit('query', ctx.query.stat(figure, 'attack', 7))
        local answer = ctx.choose { chooser = 1, prompt = 'pick', options = { 'a', 'b' } }
        ctx.emit('answer', answer)
        local vars = ctx.vars('t:probe')
        vars.n = 4
        ctx.emit('vars', vars.n)
        ctx.log('hello', 3)
        local roll = ctx.dice.roll('combat', 4)
        ctx.emit('roll', { hit = roll:count('hit'), sum = roll:sum(), faces = #roll:faces() })
      end,
    }
  end,
}

rule_module {
  id = 'broken',
  register = function(r) r.action { id = 'nope' } end,
}

rule_module {
  id = 'loops',
  register = function(r)
    r.trigger { event = 'wounds_applied', effect = function() while true do end end }
  end,
}

rule_module {
  id = 'natives',
  register = function(r)
    r.modifier { query = 'q', layer = 'add', when = { cond = 'engaged', who = 'self' }, value = 1 }
    r.action { id = 'shove', slots = {}, targets = 'enemies', resolve = function() end }
    r.trigger { event = 'wounds_applied', filter = true, effect = function() end }
  end,
}

rule_module {
  id = 'bad_effect',
  register = function(r)
    r.trigger { event = 'wounds_applied', effect = { eff = 'heel' } }
  end,
}

rule_module {
  id = 'two_faults',
  register = function(r)
    r.modifier { query = 'q', layer = 'sideways', value = 1, override = 'yes', when = 'x', zz = 1 }
  end,
}

rule_module {
  id = 'two_bad_natives',
  register = function(r)
    r.modifier { query = 'q', layer = 'add', value = 1, when = { a = 1.5, b = function() end } }
  end,
}
"#;

fn pack() -> (tempfile::TempDir, Vec<PackSource>) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("data.lua"), DATA).unwrap();
    let source = PackSource {
        id: PackId::new("t"),
        root: dir.path().to_owned(),
        dependencies: Vec::new(),
    };
    (dir, vec![source])
}

fn scripts(packs: &[PackSource]) -> Vec<strata_model::ScriptRef> {
    let records = freeze(&DataStage::run(packs).unwrap()).unwrap();
    records.into_iter().flat_map(|r| r.scripts).collect()
}

fn record(id: &str, packs: &[PackSource]) -> FrozenRecord {
    let records = freeze(&DataStage::run(packs).unwrap()).unwrap();
    records
        .into_iter()
        .find(|r| r.record.id == format!("t:{id}"))
        .unwrap()
}

fn module(id: &str, packs: &[PackSource]) -> LuaModule {
    LuaModule::new(&record(id, packs), packs, Limits::default()).unwrap()
}

/// Answers the questions `ctx` asks of the host: figure 2 is the only enemy, every stat is 2,
/// and `units.wound` wounds a figure.
struct Host {
    logged: Mutex<Vec<String>>,
}

impl RuleHost for Host {
    fn eval(
        &self,
        _: &GameState,
        d: &Descriptor,
        _: &Value,
        _: &Value,
    ) -> Result<Value, LogicError> {
        match d.0.get("sel").and_then(Value::as_str) {
            Ok("enemies") => Ok(Value::List(vec![Value::Int(2)])),
            Ok(_) => Ok(Value::List(vec![Value::Int(3), Value::Int(1)])),
            Err(e) => Err(LogicError(e.to_string())),
        }
    }

    fn query(&self, _: &GameState, name: &str, args: &[Value]) -> Result<Value, LogicError> {
        assert_eq!(name, "stat");
        Ok(Value::Int(2 + args.len() as i64))
    }

    fn die(&self, name: &str) -> Option<(DefId, DieKind)> {
        let faces = ["hit", "hit", "block"].map(Sym::new).to_vec();
        (name == "combat").then_some((DefId(3), DieKind::Faces(faces)))
    }

    fn call(&self, ns: &str, name: &str, args: &[Value], ctx: &mut dyn Ctx) -> Result<Value, Halt> {
        assert_eq!((ns, name), ("units", "wound"));
        let figure = FigureId(args[0].as_int().unwrap() as u32);
        let amount = args[1].as_int().unwrap() as u32;
        if amount > 0 {
            let wound = DefId(1);
            ctx.emit(Event::WoundsApplied {
                figure,
                wound,
                amount,
            })?;
        }
        Ok(Value::Nil)
    }

    fn log(&self, text: &str) {
        self.logged.lock().unwrap().push(text.to_owned());
    }
}

/// Registers the command that wounds figure 2 and the `units` namespace.
struct Driver;

struct WoundCommand;

impl CommandHandler for WoundCommand {
    fn kind(&self) -> &str {
        "wound"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, _: &Command) -> Result<(), Halt> {
        let (figure, wound) = (FigureId(2), DefId(1));
        ctx.emit(Event::WoundsApplied {
            figure,
            wound,
            amount: 2,
        })
    }
}

impl RuleModule for Driver {
    fn id(&self) -> ModuleId {
        ModuleId::new("t:driver")
    }

    fn slot(&self) -> Option<strata_kernel::SlotId> {
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
        r.command(WoundCommand);
        r.lua_api(
            "units",
            LuaApi {
                functions: vec![Sym::new("wound")],
            },
        );
        Ok(())
    }
}

struct World {
    rt: RulesRuntime,
    rules: LuaRules,
    host: Arc<Host>,
}

fn world(min_wounds: i64, modules: &[&str]) -> (tempfile::TempDir, World) {
    let (dir, packs) = pack();
    let mut catalog = ModuleCatalog::default();
    catalog.register(Box::new(Driver)).unwrap();
    let mut addons = vec![ModuleRef {
        id: ModuleId::new("t:driver"),
        config: BTreeMap::new(),
    }];
    let mut bindings = Vec::new();
    for name in modules {
        let id = format!("t:{name}");
        catalog.register(Box::new(module(name, &packs))).unwrap();
        let config = if *name == "defender_first" {
            BTreeMap::from([(Sym::new("min_wounds"), Value::Int(min_wounds))])
        } else {
            BTreeMap::new()
        };
        let full = Value::Map(config.clone());
        let full = if config.is_empty() {
            Value::Map(BTreeMap::new())
        } else {
            full
        };
        bindings.push(ModuleBinding {
            id: id.clone(),
            config: full,
        });
        addons.push(ModuleRef {
            id: ModuleId::new(&id),
            config,
        });
    }
    let mut ruleset: RulesetDef = serde_json::from_value(serde_json::json!({ "id": 0 })).unwrap();
    ruleset.addons = addons;
    let rulesets = BTreeMap::from([(Sym::new("rs"), ruleset)]);
    let rt = resolve("rs", &rulesets, &catalog).unwrap();
    let host = Arc::new(Host {
        logged: Mutex::default(),
    });
    let rules = LuaRules::new(&rt, scripts(&packs), packs, bindings, host.clone());
    (dir, World { rt, rules, host })
}

fn state() -> GameState {
    let figure = |controller| FigureState {
        controller: PlayerId(controller),
        ..Default::default()
    };
    GameState {
        setup: Arc::new(MatchSetup {
            seed: SeedSource::Plain(7),
            ..Default::default()
        }),
        players: vec![PlayerState::default(); 2],
        figures: vec![figure(0), figure(0), figure(1)],
        flow: strata_model::FlowState {
            round: 3,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// A `Ctx` that applies what is emitted and answers from a tape.
struct Bench {
    state: GameState,
    rng: CommandRng,
    events: Vec<Event>,
    tape: Vec<Answer>,
    asked: Vec<Decision>,
}

impl Bench {
    fn new(tape: Vec<Answer>) -> Self {
        Self {
            state: state(),
            rng: CommandRng::for_command(7, 0),
            events: Vec::new(),
            tape,
            asked: Vec::new(),
        }
    }
}

impl Ctx for Bench {
    fn state(&self) -> &GameState {
        &self.state
    }

    fn rng(&mut self) -> &mut CommandRng {
        &mut self.rng
    }

    fn emit(&mut self, e: Event) -> Result<(), Halt> {
        self.state.apply(&e)?;
        self.events.push(e);
        Ok(())
    }

    fn window(&mut self, _: &strata_kernel::WindowId) -> Result<(), Halt> {
        Ok(())
    }

    fn choose(&mut self, chooser: PlayerId, d: Decision) -> Result<Answer, Halt> {
        self.asked.push(d.clone());
        match self.tape.get(self.asked.len() - 1) {
            Some(answer) => Ok(answer.clone()),
            None => Err(Halt::NeedsDecision {
                decision: d,
                chooser,
            }),
        }
    }
}

fn customs(events: &[Event]) -> Vec<(String, Value)> {
    let custom = |e: &Event| match e {
        Event::Custom { id, payload } => Some((id.to_string(), payload.clone())),
        _ => None,
    };
    events.iter().filter_map(custom).collect()
}

fn map(entries: &[(&str, Value)]) -> Value {
    Value::Map(
        entries
            .iter()
            .map(|(k, v)| (Sym::new(k), v.clone()))
            .collect(),
    )
}

#[test]
fn a_lua_module_registers_an_action_a_modifier_and_a_trigger() {
    let (_dir, w) = world(1, &["defender_first"]);
    let paths = |logic: &Logic| match logic {
        Logic::Script(s) => s.path.to_string(),
        other => panic!("not a script: {other:?}"),
    };
    let action = &w.rt.actions()[0];
    assert_eq!(action.id.as_str(), "attack");
    assert_eq!(
        paths(&action.resolve),
        "rule_module:defender_first/~registered/1/def/resolve"
    );
    assert_eq!(
        paths(action.targets.as_ref().unwrap()),
        "rule_module:defender_first/~registered/1/def/targets"
    );
    assert_eq!(w.rt.modifiers().len(), 1);
    assert_eq!(
        paths(&w.rt.triggers()[0].effect),
        "rule_module:defender_first/~registered/3/def/effect"
    );
    assert_eq!(w.rt.windows().len(), 1);
}

#[test]
fn the_action_behaves_like_its_rust_equivalent() {
    let (_dir, w) = world(1, &["defender_first"]);
    let action = &w.rt.actions()[0];
    let mut bench = Bench::new(Vec::new());
    let targets = w.rules.targets(action, FigureId(0), &bench.state).unwrap();
    assert_eq!(targets, [Value::Int(2)]);
    w.rules
        .resolve(action, FigureId(0), &Value::Int(2), &mut bench)
        .unwrap();

    // The same rolls made in Rust: defender (stat 3 dice) first, then the attacker (3 dice).
    let faces = |rng: &mut CommandRng, n| (0..n).map(|_| rng.roll(3)).collect::<Vec<_>>();
    let mut rng = CommandRng::for_command(7, 0);
    let defense = faces(&mut rng, 4);
    let attack = faces(&mut rng, 3);
    let hits = attack.iter().filter(|r| **r <= 2).count() as i64;
    let blocks = defense.iter().filter(|r| **r == 3).count() as i64;
    let wounds = (hits - blocks).max(1) as u32;
    assert_eq!(
        bench.events,
        [
            Event::DiceRolled {
                die: DefId(3),
                results: defense
            },
            Event::DiceRolled {
                die: DefId(3),
                results: attack
            },
            Event::WoundsApplied {
                figure: FigureId(2),
                wound: DefId(1),
                amount: wounds
            },
        ]
    );
}

#[test]
fn the_modifier_applies_through_the_query_engine() {
    let (_dir, w) = world(1, &["defender_first"]);
    let mods: Vec<SourcedModifier> =
        w.rt.modifiers()
            .iter()
            .map(|def| SourcedModifier {
                source: Sym::new("t:defender_first"),
                priority: 0,
                def: def.clone(),
            })
            .collect();
    let bounds = BTreeMap::from([(Sym::new("defense"), StatBounds { min: 0, max: 10 })]);
    let query = StatQuery {
        stat: Sym::new("defense"),
        base: 3,
        subject: Value::Int(2),
        context: Value::Nil,
    };
    struct NoNative;
    impl LogicEvaluator for NoNative {
        fn eval(&self, _: &Descriptor, _: &strata_rules::EvalCtx<'_>) -> Result<Value, LogicError> {
            Err(LogicError("no native logic".into()))
        }
    }
    let defense = w
        .rules
        .with_state(|rt| {
            let logics = Logics {
                evaluator: &NoNative,
                scripts: rt,
            };
            QueryEngine::new(&mods, &bounds, logics).query(&query)
        })
        .unwrap()
        .unwrap();
    assert_eq!(defense, 3 + 1 + 1);
}

fn run_wound(w: &World) -> (Outcome, GameState, u64) {
    let mut engine = Engine::new(&w.rt, &w.rules, state());
    let command = Command::Custom {
        id: Sym::new("wound"),
        payload: Value::Nil,
    };
    let outcome = engine.submit(PlayerId(0), command);
    (
        outcome,
        engine.state().clone(),
        engine.log().first().map_or(0, |r| r.state_hash),
    )
}

#[test]
fn the_trigger_runs_in_the_engine_and_replays_identically() {
    let (_a, first) = world(0, &["defender_first"]);
    let (_b, second) = world(0, &["defender_first"]);
    let (outcome, state, hash) = run_wound(&first);
    assert!(matches!(outcome, Outcome::Committed(_)), "{outcome:?}");
    let hits = VarKey {
        module: ModuleId::new("t:defender_first"),
        name: Sym::new("hits"),
    };
    assert_eq!(state.vars.get(&hits), Some(&Value::Int(2)));
    assert_eq!(run_wound(&second).2, hash);
}

#[test]
fn every_ctx_entry_point_works() {
    let (_dir, w) = world(0, &["probe"]);
    let action = &w.rt.actions()[0];
    let mut bench = Bench::new(vec![Answer::Option(1)]);
    w.rules
        .resolve(action, FigureId(0), &Value::Nil, &mut bench)
        .unwrap();
    let out = customs(&bench.events);
    let get = |name: &str| out.iter().find(|(id, _)| id == name).unwrap().1.clone();
    let state = map(&[
        ("writable", Value::Bool(false)),
        ("round", Value::Int(3)),
        ("figures", Value::Int(3)),
        ("active", Value::Int(0)),
    ]);
    assert_eq!(get("state"), state);
    assert_eq!(
        get("select"),
        Value::List(vec![Value::Int(1), Value::Int(3)])
    );
    assert_eq!(get("query"), Value::Int(5));
    assert_eq!(get("answer"), Value::Str(Sym::new("b")));
    assert_eq!(get("vars"), Value::Int(4));
    let probe = VarKey {
        module: ModuleId::new("t:probe"),
        name: Sym::new("n"),
    };
    assert_eq!(bench.state.vars.get(&probe), Some(&Value::Int(4)));
    assert_eq!(*w.host.logged.lock().unwrap(), ["hello 3"]);
    let Value::Map(roll) = get("roll") else {
        panic!("roll")
    };
    assert_eq!(roll["faces"], Value::Int(4));
    let hit = roll["hit"].as_int().unwrap();
    assert!((0..=4).contains(&hit) && roll["sum"].as_int().unwrap() >= 4);
    assert_eq!(bench.asked.len(), 1);
}

#[test]
fn an_unanswered_choose_halts_and_the_answer_resumes_it() {
    let (_dir, w) = world(0, &["probe"]);
    let action = &w.rt.actions()[0];
    let mut bench = Bench::new(Vec::new());
    let halted = w
        .rules
        .resolve(action, FigureId(0), &Value::Nil, &mut bench);
    let Err(Halt::NeedsDecision { decision, chooser }) = halted else {
        panic!("{halted:?}")
    };
    assert_eq!(chooser, PlayerId(1));
    assert_eq!(decision.prompt.as_str(), "pick");
}

#[test]
fn a_bad_registration_is_a_module_error() {
    let (_dir, packs) = pack();
    let broken = module("broken", &packs);
    let error = broken.register(&mut Registrar::default(), &Value::Map(BTreeMap::new()));
    let ModuleError::Invalid(message) = error.unwrap_err();
    assert!(
        message.contains("t:broken") && message.contains("r.action"),
        "{message}"
    );
}

#[test]
fn an_unknown_native_descriptor_is_a_module_error_when_it_registers() {
    let (_dir, packs) = pack();
    let error = module("bad_effect", &packs).register(&mut Registrar::default(), &map(&[]));
    let ModuleError::Invalid(message) = error.unwrap_err();
    assert!(message.contains("r.trigger"), "{message}");
    assert!(
        message.contains("`effect`: unknown descriptor `eff.heel`"),
        "{message}"
    );
}

#[test]
fn a_runaway_script_rejects_the_command() {
    let (_dir, w) = world(0, &["loops"]);
    let (outcome, ..) = run_wound(&w);
    let Outcome::Rejected(violation) = outcome else {
        panic!("{outcome:?}")
    };
    assert!(violation.0.contains("budget"), "{violation}");
}

#[test]
fn a_runtime_state_must_run_register_to_find_registered_functions() {
    let (_dir, packs) = pack();
    let plain = RuntimeLua::new(&scripts(&packs), &packs).unwrap();
    assert!(
        plain
            .call_script(
                &strata_model::ScriptRef {
                    pack: PackId::new("t"),
                    def: DefId(0),
                    path: Sym::new("rule_module:probe/~registered/1/def/resolve"),
                },
                &[]
            )
            .is_err()
    );
}

#[test]
fn plain_data_in_a_registration_is_native_logic() {
    let (_dir, w) = world(0, &["natives"]);
    let native = |value: Value| Some(Logic::Native(Descriptor(value)));
    let modifier = &w.rt.modifiers()[0];
    let adjacent = map(&[
        ("cond", Value::Str(Sym::new("engaged"))),
        ("who", Value::Str(Sym::new("self"))),
    ]);
    assert_eq!(modifier.when, native(adjacent));
    assert_eq!(Some(modifier.value.clone()), native(Value::Int(1)));
    let action = &w.rt.actions()[0];
    assert_eq!(action.targets, native(Value::Str(Sym::new("enemies"))));
    assert!(matches!(action.resolve, Logic::Script(_)));
    let trigger = &w.rt.triggers()[0];
    assert_eq!(trigger.filter, native(Value::Bool(true)));
}

#[test]
fn a_bad_registration_reports_the_same_error_in_every_fresh_state() {
    let (_dir, packs) = pack();
    let message = || {
        let error = module("two_faults", &packs).register(&mut Registrar::default(), &map(&[]));
        let ModuleError::Invalid(message) = error.unwrap_err();
        message
    };
    let first = message();
    assert!(first.contains("r.modifier"), "{first}");
    for _ in 0..16 {
        assert_eq!(message(), first);
    }
}

#[test]
fn a_native_descriptor_with_two_faults_reports_the_same_one_in_every_fresh_state() {
    let (_dir, packs) = pack();
    let message = || {
        let module = module("two_bad_natives", &packs);
        let error = module.register(&mut Registrar::default(), &map(&[]));
        let ModuleError::Invalid(message) = error.unwrap_err();
        message
    };
    let first = message();
    assert!(first.contains("a number is not plain data"), "{first}");
    for _ in 0..16 {
        assert_eq!(message(), first);
    }
}
