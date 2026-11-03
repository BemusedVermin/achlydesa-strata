//! `core:activation`: ordering, early end and granted slots, driven through the engine.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::json;
use strata_kernel::{ModuleId, SlotId, Sym, Value};
use strata_model::{
    Command, Event, FigureId, FigureState, FigureStatus, GameState, PlayerId, PlayerState,
    ReactionDef, ReplacementDef, RulesetDef, TriggerDef, UnitId, UnitState,
};
use strata_modules::activation::{Activation, GRANT_SLOT};
use strata_rules::{
    ActivationRules, CommandHandler, ConfigSchema, Ctx, Engine, Halt, HookRunner, ModuleCatalog,
    ModuleError, OrderingPolicy, Outcome, Registrar, RuleModule, RulesRuntime, resolve,
};

fn sym(s: &str) -> Sym {
    Sym::new(s)
}

/// Mirrors the configuration given to `core:activation`, for the test commands.
fn rules(all_figures: bool) -> ActivationRules {
    ActivationRules {
        steps: vec![sym("move"), sym("attack")],
        budgets: BTreeMap::new(),
        ordering: if all_figures {
            OrderingPolicy::AllFiguresPerStep
        } else {
            OrderingPolicy::PerFigure
        },
    }
}

fn payload_of(command: &Command) -> Result<&Value, Halt> {
    match command {
        Command::Custom { payload, .. } => Ok(payload),
        _ => Err(Halt::reject("custom only")),
    }
}

/// `spend { figure, slot }` spends a slot, as an action would.
struct Spend(ActivationRules);

impl CommandHandler for Spend {
    fn kind(&self) -> &str {
        "spend"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, command: &Command) -> Result<(), Halt> {
        let payload = payload_of(command)?;
        let figure = FigureId(payload.get("figure").unwrap().as_int().unwrap() as u32);
        let slot = sym(payload.get("slot").unwrap().as_str().unwrap());
        self.0.consume(ctx, figure, &[slot])
    }
}

/// `grant { figure, slot, extra }` emits the event an ability would.
struct Grant;

impl CommandHandler for Grant {
    fn kind(&self) -> &str {
        "grant"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, command: &Command) -> Result<(), Halt> {
        let payload = payload_of(command)?.clone();
        let id = sym(GRANT_SLOT);
        ctx.emit(Event::Custom { id, payload })
    }
}

struct Harness(ActivationRules);

impl RuleModule for Harness {
    fn id(&self) -> ModuleId {
        ModuleId::new("t:harness")
    }

    fn slot(&self) -> Option<SlotId> {
        None
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        static EMPTY: OnceLock<ConfigSchema> = OnceLock::new();
        EMPTY.get_or_init(ConfigSchema::default)
    }

    fn register(&self, r: &mut Registrar, _: &Value) -> Result<(), ModuleError> {
        r.command(Spend(self.0.clone()));
        r.command(Grant);
        Ok(())
    }
}

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
        Err(Halt::reject("only native triggers are registered"))
    }

    fn reactor(&self, _: &ReactionDef, _: &GameState) -> Result<Option<PlayerId>, Halt> {
        Ok(None)
    }

    fn react(&self, _: &ReactionDef, _: &mut dyn Ctx) -> Result<(), Halt> {
        Ok(())
    }
}

fn runtime(all_figures: bool) -> RulesRuntime {
    let config = json!({ "all_figures_per_step": { "Bool": all_figures } });
    let body = json!({
        "id": 0,
        "slots": { "activation": { "id": "core:activation", "config": config } },
        "addons": [{ "id": "t:harness" }],
    });
    let sets: BTreeMap<Sym, RulesetDef> =
        [(sym("rs"), serde_json::from_value(body).unwrap())].into();
    let mut catalog = ModuleCatalog::default();
    catalog.register(Box::new(Activation)).unwrap();
    let harness = Harness(rules(all_figures));
    catalog.register(Box::new(harness)).unwrap();
    resolve("rs", &sets, &catalog).unwrap()
}

/// Player 0 has a squad of two figures (unit 0); player 1 has one figure (unit 1).
fn state() -> GameState {
    let figure = |unit: u32, controller: u8| FigureState {
        unit: UnitId(unit),
        controller: PlayerId(controller),
        status: FigureStatus::OnBoard,
        ..Default::default()
    };
    let unit = |owner: u8, figures: Vec<FigureId>| UnitState {
        owner: PlayerId(owner),
        figures,
        ..Default::default()
    };
    GameState {
        players: vec![PlayerState::default(); 2],
        units: vec![
            unit(0, vec![FigureId(0), FigureId(1)]),
            unit(1, vec![FigureId(2)]),
        ],
        figures: vec![figure(0, 0), figure(0, 0), figure(1, 1)],
        ..Default::default()
    }
}

fn custom(id: &str, figure: u32, slot: &str, extra: i64) -> Command {
    let field = |k: &str, v| (sym(k), v);
    let payload = Value::Map(
        [
            field("figure", Value::Int(figure.into())),
            field("slot", Value::Str(sym(slot))),
            field("extra", Value::Int(extra)),
        ]
        .into(),
    );
    Command::Custom {
        id: sym(id),
        payload,
    }
}

fn spend(figure: u32, slot: &str) -> Command {
    custom("spend", figure, slot, 0)
}

fn ok(outcome: Outcome) -> Vec<Event> {
    match outcome {
        Outcome::Committed(events) => events,
        other => panic!("expected a commit, got {other:?}"),
    }
}

fn refused(outcome: Outcome) {
    assert!(matches!(outcome, Outcome::Rejected(_)), "{outcome:?}");
}

fn start(rt: &RulesRuntime) -> Engine<'_> {
    let mut engine = Engine::new(rt, &NoHooks, state());
    ok(engine.submit(PlayerId(0), Command::ActivateUnit(UnitId(0))));
    engine
}

fn left(engine: &Engine, figure: u32, slot: &str) -> u8 {
    let act = engine.state().flow.activation.as_ref().unwrap();
    act.slots[&FigureId(figure)][&sym(slot)]
}

#[test]
fn a_squad_moves_all_figures_before_any_attacks_under_the_default_config() {
    let rt = runtime(true);
    let mut engine = start(&rt);
    let p0 = PlayerId(0);
    // Each figure begins with one move and one attack.
    assert_eq!(
        (left(&engine, 0, "move"), left(&engine, 1, "attack")),
        (1, 1)
    );
    refused(engine.submit(p0, spend(0, "attack")));
    ok(engine.submit(p0, spend(0, "move")));
    // Figure 1 has not moved, so figure 0 may not attack yet.
    refused(engine.submit(p0, spend(0, "attack")));
    refused(engine.submit(p0, spend(1, "attack")));
    ok(engine.submit(p0, spend(1, "move")));
    let step = engine
        .state()
        .flow
        .activation
        .as_ref()
        .unwrap()
        .step
        .clone();
    assert_eq!(step, sym("attack"));
    ok(engine.submit(p0, spend(1, "attack")));
    ok(engine.submit(p0, spend(0, "attack")));
    refused(engine.submit(p0, spend(0, "attack")));
}

#[test]
fn with_all_figures_per_step_off_one_figure_finishes_before_the_next_starts() {
    let rt = runtime(false);
    let mut engine = start(&rt);
    let p0 = PlayerId(0);
    ok(engine.submit(p0, spend(0, "move")));
    refused(engine.submit(p0, spend(1, "move")));
    ok(engine.submit(p0, spend(0, "attack")));
    ok(engine.submit(p0, spend(1, "move")));
}

#[test]
fn an_activation_can_end_early_and_then_nothing_is_spent() {
    let rt = runtime(true);
    let mut engine = start(&rt);
    let p0 = PlayerId(0);
    ok(engine.submit(p0, spend(0, "move")));
    let events = ok(engine.submit(p0, Command::EndActivation));
    assert!(events.contains(&Event::ActivationEnded));
    assert!(engine.state().flow.activation.is_none());
    refused(engine.submit(p0, spend(1, "move")));
    refused(engine.submit(p0, Command::EndActivation));
}

#[test]
fn only_the_active_player_may_activate_or_end() {
    let rt = runtime(true);
    let mut engine = Engine::new(&rt, &NoHooks, state());
    let p1 = PlayerId(1);
    refused(engine.submit(p1, Command::ActivateUnit(UnitId(1))));
    ok(engine.submit(PlayerId(0), Command::ActivateUnit(UnitId(0))));
    refused(engine.submit(p1, Command::EndActivation));
    refused(engine.submit(PlayerId(0), Command::ActivateUnit(UnitId(0))));
}

#[test]
fn an_ability_can_grant_an_extra_move_after_the_moves_are_spent() {
    let rt = runtime(true);
    let mut engine = start(&rt);
    let p0 = PlayerId(0);
    ok(engine.submit(p0, spend(0, "move")));
    ok(engine.submit(p0, spend(1, "move")));
    refused(engine.submit(p0, spend(0, "move")));
    ok(engine.submit(p0, spend(0, "attack")));
    ok(engine.submit(p0, custom("grant", 0, "move", 1)));
    assert_eq!(left(&engine, 0, "move"), 1);
    // The granted move reopens the move step; figure 1's attack waits for it.
    refused(engine.submit(p0, spend(1, "attack")));
    ok(engine.submit(p0, spend(0, "move")));
    ok(engine.submit(p0, spend(1, "attack")));
}

#[test]
fn granting_to_a_figure_outside_the_activation_is_refused() {
    let rt = runtime(true);
    let mut engine = start(&rt);
    refused(engine.submit(PlayerId(0), custom("grant", 2, "move", 1)));
}

#[test]
fn the_same_commands_twice_give_the_same_hashes() {
    let play = || {
        let rt = runtime(true);
        let mut engine = start(&rt);
        let p0 = PlayerId(0);
        ok(engine.submit(p0, spend(0, "move")));
        ok(engine.submit(p0, custom("grant", 0, "move", 1)));
        ok(engine.submit(p0, Command::EndActivation));
        let hashes = engine.log().iter().map(|r| r.state_hash);
        hashes.collect::<Vec<_>>()
    };
    assert_eq!(play(), play());
}
