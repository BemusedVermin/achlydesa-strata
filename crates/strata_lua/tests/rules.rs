use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use strata_kernel::{CommandRng, DefId, ModuleId, PackId, SlotId, Sym, Value, WindowId};
use strata_lua::{DataStage, FrozenDef, LuaRules, PackSource, RuleHost, freeze};
use strata_model::{
    AbilityDef, Answer, Command, Decision, Descriptor, DieKind, Event, FigureId, FigureState,
    FlowState, GameState, Logic, MatchSetup, ModuleRef, PlayerId, PlayerState, RulesetDef,
    ScriptRef, SeedSource, VarKey,
};
use strata_rules::{
    ActionRunner, CommandHandler, ConfigSchema, Ctx, Engine, EvalCtx, Halt, HookRunner, LogicError,
    LogicEvaluator, Logics, LuaApi, ModuleCatalog, ModuleError, Outcome, QueryEngine, Registrar,
    RuleModule, RulesRuntime, SourcedModifier, StatBounds, StatQuery, resolve,
};

const DATA: &str = r#"
ability {
  id = 'brace', name = 'Brace', text = 'Test.', scope = 'unit',
  actions = {{
    id = 'attack',
    slots = { 'attack' },
    targets = function(ctx, attacker) return ctx.select({ sel = 'enemies' }, attacker) end,
    resolve = function(ctx, attacker, target)
      local def = ctx.dice.roll('combat', ctx.query.stat(target, 'defense'))
      local atk = ctx.dice.roll('combat', 3)
      ctx.units.wound(target, math.max(1, atk:count('hit') - def:count('block')))
    end,
  }},
  modifiers = {{ query = 'defense', layer = 'add', value = function() return 1 end }},
  triggers = {{
    event = 'wounds_applied',
    effect = function(ctx, event)
      local vars = ctx.vars('t:brace')
      vars.hits = (vars.hits or 0) + event.WoundsApplied.amount
    end,
  }},
  replacements = {{
    event = 'var_set',
    filter = function(ctx, event) return event.VarSet.value.Int == 13 end,
    replace = function() return {} end,
  }},
  reactions = {{
    window = 'w',
    when = function(ctx) return 1 end,
    effect = function(ctx) ctx.emit('reacted', 1) end,
  }},
}

ability {
  id = 'probe', name = 'Probe', text = 'Test.', scope = 'unit',
  actions = {{
    id = 'probe',
    slots = {},
    resolve = function(ctx, figure)
      local writable = pcall(function() ctx.state.round = 9 end)
      local deep = pcall(function() ctx.state.figures[1].status = 'destroyed' end)
      ctx.emit('state', { writable = writable or deep, round = ctx.state.round,
                          figures = #ctx.state.figures, active = ctx.state.active })
      ctx.emit('select', ctx.select({ sel = 'allies' }))
      ctx.emit('query', ctx.query.stat(figure, 'attack', 7))
      ctx.emit('answer', ctx.choose { chooser = 1, prompt = 'pick', options = { 'a', 'b' } })
      local vars = ctx.vars('t:probe')
      vars.n = 4
      ctx.emit('vars', vars.n)
      ctx.log('hello', 3)
      local roll = ctx.dice.roll('combat', 4)
      ctx.emit('roll', { hit = roll:count('hit'), sum = roll:sum(), faces = #roll:faces() })
    end,
  }},
}

ability {
  id = 'loops', name = 'Loops', text = 'Test.', scope = 'unit',
  triggers = {{ event = 'wounds_applied', effect = function() while true do end end }},
}
"#;

/// The pack on disk, the abilities in it and every script the freeze found.
struct Content {
    _dir: tempfile::TempDir,
    packs: Vec<PackSource>,
    abilities: BTreeMap<String, AbilityDef>,
    scripts: Vec<ScriptRef>,
}

fn content() -> Content {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("data.lua"), DATA).unwrap();
    let source = PackSource {
        id: PackId::new("t"),
        root: dir.path().to_owned(),
        dependencies: Vec::new(),
    };
    let packs = vec![source];
    let records = freeze(&DataStage::run(&packs).unwrap()).unwrap();
    let mut abilities = BTreeMap::new();
    let mut scripts = Vec::new();
    for record in records {
        scripts.extend(record.scripts);
        if let FrozenDef::Ability(def) = record.def {
            abilities.insert(record.record.id.clone(), def);
        }
    }
    Content {
        _dir: dir,
        packs,
        abilities,
        scripts,
    }
}

/// Answers the questions `ctx` asks of the host: figure 2 is the only enemy, a stat query
/// answers `2 + the number of arguments`, and `units.wound` wounds a figure.
struct Host {
    logged: Mutex<Vec<String>>,
}

impl RuleHost for Host {
    fn eval(&self, _: &GameState, d: &Descriptor, _: &Value) -> Result<Value, LogicError> {
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
        let wound = DefId(1);
        ctx.emit(Event::WoundsApplied {
            figure,
            wound,
            amount,
        })?;
        Ok(Value::Nil)
    }

    fn log(&self, text: &str) {
        self.logged.lock().unwrap().push(text.to_owned());
    }
}

struct WoundCommand;

impl CommandHandler for WoundCommand {
    fn kind(&self) -> &str {
        "wound"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, _: &Command) -> Result<(), Halt> {
        let wound = DefId(1);
        let figure = FigureId(2);
        ctx.emit(Event::WoundsApplied {
            figure,
            wound,
            amount: 2,
        })
    }
}

/// Registers the `wound` command, the `units` namespace and the triggers of some abilities.
struct Driver(Vec<AbilityDef>);

impl RuleModule for Driver {
    fn id(&self) -> ModuleId {
        ModuleId::new("t:driver")
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
        r.command(WoundCommand);
        let functions = vec![Sym::new("wound")];
        r.lua_api("units", LuaApi { functions });
        for ability in &self.0 {
            ability.triggers.iter().for_each(|t| r.trigger(t.clone()));
        }
        Ok(())
    }
}

struct World {
    content: Content,
    rt: RulesRuntime,
    rules: LuaRules,
    host: Arc<Host>,
}

impl World {
    fn ability(&self, id: &str) -> &AbilityDef {
        &self.content.abilities[&format!("t:{id}")]
    }
}

/// A world whose engine runs the triggers of the abilities named in `triggers`.
fn world(triggers: &[&str]) -> World {
    let content = content();
    let chosen = triggers
        .iter()
        .map(|id| content.abilities[&format!("t:{id}")].clone());
    let mut catalog = ModuleCatalog::default();
    catalog
        .register(Box::new(Driver(chosen.collect())))
        .unwrap();
    let mut ruleset: RulesetDef = serde_json::from_value(serde_json::json!({ "id": 0 })).unwrap();
    ruleset.addons = vec![ModuleRef {
        id: ModuleId::new("t:driver"),
        config: BTreeMap::new(),
    }];
    let rulesets = BTreeMap::from([(Sym::new("rs"), ruleset)]);
    let rt = resolve("rs", &rulesets, &catalog).unwrap();
    let host = Arc::new(Host {
        logged: Mutex::default(),
    });
    let rules = LuaRules::new(
        &rt,
        content.scripts.clone(),
        content.packs.clone(),
        host.clone(),
    );
    World {
        content,
        rt,
        rules,
        host,
    }
}

/// Figures 0 and 1 belong to player 0, figure 2 to player 1; it is round 3.
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
        flow: FlowState {
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

    fn window(&mut self, _: &WindowId) -> Result<(), Halt> {
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

fn customs(events: &[Event]) -> BTreeMap<String, Value> {
    let custom = |e: &Event| match e {
        Event::Custom { id, payload } => Some((id.to_string(), payload.clone())),
        _ => None,
    };
    events.iter().filter_map(custom).collect()
}

fn map(entries: &[(&str, Value)]) -> Value {
    let entry = |(k, v): &(&str, Value)| (Sym::new(k), v.clone());
    Value::Map(entries.iter().map(entry).collect())
}

fn ints(items: &[i64]) -> Value {
    Value::List(items.iter().map(|i| Value::Int(*i)).collect())
}

#[test]
fn an_action_behaves_like_its_rust_equivalent() {
    let w = world(&[]);
    let action = &w.ability("brace").actions[0];
    let mut bench = Bench::new(Vec::new());
    let targets = w.rules.targets(action, FigureId(0), &bench.state).unwrap();
    assert_eq!(targets, [Value::Int(2)]);
    w.rules
        .resolve(action, FigureId(0), &Value::Int(2), &mut bench)
        .unwrap();

    // The same rolls made in Rust: the defender first, with the 4 dice the stat query gives.
    let mut rng = CommandRng::for_command(7, 0);
    let mut throw = |n| (0..n).map(|_| rng.roll(3)).collect::<Vec<_>>();
    let (defense, attack) = (throw(4), throw(3));
    let hits = attack.iter().filter(|r| **r <= 2).count() as i64;
    let blocks = defense.iter().filter(|r| **r == 3).count() as i64;
    let amount = (hits - blocks).max(1) as u32;
    let die = DefId(3);
    assert_eq!(
        bench.events,
        [
            Event::DiceRolled {
                die,
                results: defense
            },
            Event::DiceRolled {
                die,
                results: attack
            },
            Event::WoundsApplied {
                figure: FigureId(2),
                wound: DefId(1),
                amount
            },
        ]
    );
}

#[test]
fn a_modifier_applies_through_the_query_engine() {
    let w = world(&[]);
    let def = w.ability("brace").modifiers[0].clone();
    let modifiers = [SourcedModifier {
        source: Sym::new("t:brace"),
        priority: 0,
        def,
    }];
    let bounds = BTreeMap::from([(Sym::new("defense"), StatBounds { min: 0, max: 10 })]);
    let query = StatQuery {
        stat: Sym::new("defense"),
        base: 3,
        subject: Value::Int(2),
        context: Value::Nil,
    };
    struct NoNative;
    impl LogicEvaluator for NoNative {
        fn eval(&self, _: &Descriptor, _: &EvalCtx<'_>) -> Result<Value, LogicError> {
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
            QueryEngine::new(&modifiers, &bounds, logics).query(&query)
        })
        .unwrap();
    assert_eq!(defense.unwrap(), 4);
}

fn run_wound(w: &World) -> (Outcome, GameState, u64) {
    let mut engine = Engine::new(&w.rt, &w.rules, state());
    let id = Sym::new("wound");
    let outcome = engine.submit(
        PlayerId(0),
        Command::Custom {
            id,
            payload: Value::Nil,
        },
    );
    let hash = engine.log().first().map_or(0, |r| r.state_hash);
    (outcome, engine.state().clone(), hash)
}

#[test]
fn a_trigger_runs_in_the_engine_and_replays_identically() {
    let first = world(&["brace"]);
    let second = world(&["brace"]);
    let (outcome, state, hash) = run_wound(&first);
    assert!(matches!(outcome, Outcome::Committed(_)), "{outcome:?}");
    let name = Sym::new("hits");
    let hits = VarKey {
        module: ModuleId::new("t:brace"),
        name,
    };
    assert_eq!(state.vars.get(&hits), Some(&Value::Int(2)));
    assert_ne!(hash, 0);
    assert_eq!(run_wound(&second).2, hash);
}

#[test]
fn replacements_and_reactions_run_in_lua() {
    let w = world(&[]);
    let brace = w.ability("brace");
    let set = |value| Event::VarSet {
        key: VarKey::default(),
        value: Value::Int(value),
    };
    let def = &brace.replacements[0];
    let state = state();
    assert!(w.rules.replaces(def, &set(13), &state).unwrap());
    assert!(!w.rules.replaces(def, &set(1), &state).unwrap());
    let mut bench = Bench::new(Vec::new());
    assert_eq!(w.rules.replace(def, &set(13), &mut bench).unwrap(), []);
    let reaction = &brace.reactions[0];
    assert_eq!(
        w.rules.reactor(reaction, &state).unwrap(),
        Some(PlayerId(1))
    );
    w.rules.react(reaction, &mut bench).unwrap();
    assert_eq!(customs(&bench.events)["reacted"], Value::Int(1));
}

#[test]
fn every_ctx_entry_point_works() {
    let w = world(&[]);
    let action = &w.ability("probe").actions[0];
    let mut bench = Bench::new(vec![Answer::Option(1)]);
    let result = w
        .rules
        .resolve(action, FigureId(0), &Value::Nil, &mut bench);
    result.unwrap();
    let out = customs(&bench.events);
    let state = map(&[
        ("writable", Value::Bool(false)),
        ("round", Value::Int(3)),
        ("figures", Value::Int(3)),
        ("active", Value::Int(0)),
    ]);
    assert_eq!(out["state"], state);
    assert_eq!(out["select"], ints(&[1, 3]));
    assert_eq!(out["query"], Value::Int(5));
    assert_eq!(out["answer"], Value::Str(Sym::new("b")));
    assert_eq!(out["vars"], Value::Int(4));
    let n = VarKey {
        module: ModuleId::new("t:probe"),
        name: Sym::new("n"),
    };
    assert_eq!(bench.state.vars.get(&n), Some(&Value::Int(4)));
    assert_eq!(*w.host.logged.lock().unwrap(), ["hello 3"]);
    let Value::Map(roll) = &out["roll"] else {
        panic!("roll")
    };
    assert_eq!(roll["faces"], Value::Int(4));
    assert!(roll["hit"].as_int().unwrap() <= 4 && roll["sum"].as_int().unwrap() >= 4);
}

#[test]
fn an_unanswered_choose_halts_the_command() {
    let w = world(&[]);
    let action = &w.ability("probe").actions[0];
    let mut bench = Bench::new(Vec::new());
    let halted = w
        .rules
        .resolve(action, FigureId(0), &Value::Nil, &mut bench);
    let Err(Halt::NeedsDecision { decision, chooser }) = halted else {
        panic!("{halted:?}")
    };
    assert_eq!((chooser, decision.prompt.as_str()), (PlayerId(1), "pick"));
}

#[test]
fn a_runaway_script_rejects_the_command() {
    let w = world(&["loops"]);
    let (outcome, ..) = run_wound(&w);
    let Outcome::Rejected(violation) = outcome else {
        panic!("{outcome:?}")
    };
    assert!(violation.0.contains("budget"), "{violation}");
}

#[test]
fn a_native_filter_goes_through_the_host() {
    let w = world(&[]);
    let native = Logic::Native(Descriptor(map(&[("sel", Value::Str(Sym::new("enemies")))])));
    let def = strata_model::ReplacementDef {
        event: Sym::new("var_set"),
        filter: Some(native),
        replace: Logic::Native(Descriptor(Value::Nil)),
    };
    let event = Event::Custom {
        id: Sym::new("x"),
        payload: Value::Nil,
    };
    let held = w.rules.replaces(&def, &event, &state());
    assert!(
        matches!(held, Err(Halt::Rejected(_))),
        "a list is not a bool"
    );
}
