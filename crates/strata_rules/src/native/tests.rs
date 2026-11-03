use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::json;
use strata_kernel::{ModuleId, SlotId, Sym, Value};
use strata_model::{
    Command, Descriptor, Event, GameState, Logic, MatchSetup, PlayerId, ReactionDef,
    ReplacementDef, RulesetDef, SeedSource, TriggerDef,
};

use crate::*;

/// Emits `Custom(echoed, 7)` whatever set it off.
struct Echo;

impl NativeLogic for Echo {
    fn run(&self, ctx: &mut dyn Ctx, _: &Event) -> Result<(), Halt> {
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
