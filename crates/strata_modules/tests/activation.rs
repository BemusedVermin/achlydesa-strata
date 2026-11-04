//! `core:activation`'s gate, driven through the engine. Ordering, early end and granted slots are
//! pack tests: see `fixtures/packs/actions` and `fixtures/packs/activation`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::json;
use strata_kernel::{ModuleId, SlotId, Sym, Value};
use strata_model::{
    Command, Event, FigureId, FigureState, FigureStatus, GameState, PlayerId, PlayerState,
    ReactionDef, ReplacementDef, RulesetDef, TriggerDef, UnitId, UnitState,
};
use strata_modules::activation::Activation;
use strata_rules::{
    ActivationGate, ConfigSchema, Ctx, Engine, Halt, HookRunner, ModuleCatalog, ModuleError,
    Outcome, Registrar, RuleModule, RulesRuntime, resolve,
};

fn sym(s: &str) -> Sym {
    Sym::new(s)
}

/// Refuses to activate unit 0, as a token gate would for a unit without the turn's token.
struct Deny;

impl ActivationGate for Deny {
    fn may_activate(&self, _: &GameState, _: PlayerId, unit: UnitId) -> bool {
        unit != UnitId(0)
    }
}

/// Installs the `Deny` gate.
struct Harness;

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
        r.service::<dyn ActivationGate>(Box::new(Deny));
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

fn build(gated: bool) -> RulesRuntime {
    let body = json!({
        "id": 0,
        "slots": { "activation": { "id": "core:activation" } },
        "addons": if gated { json!([{ "id": "t:harness" }]) } else { json!([]) },
    });
    let sets: BTreeMap<Sym, RulesetDef> =
        [(sym("rs"), serde_json::from_value(body).unwrap())].into();
    let mut catalog = ModuleCatalog::default();
    catalog.register(Box::new(Activation)).unwrap();
    catalog.register(Box::new(Harness)).unwrap();
    resolve("rs", &sets, &catalog).unwrap()
}

/// Two units for player 0 (unit 0 has two figures, unit 1 one).
fn state() -> GameState {
    let figure = |unit: u32| FigureState {
        unit: UnitId(unit),
        controller: PlayerId(0),
        status: FigureStatus::OnBoard,
        ..Default::default()
    };
    let unit = |figures: Vec<FigureId>| UnitState {
        owner: PlayerId(0),
        figures,
        ..Default::default()
    };
    GameState {
        players: vec![PlayerState::default(); 2],
        units: vec![
            unit(vec![FigureId(0), FigureId(1)]),
            unit(vec![FigureId(2)]),
        ],
        figures: vec![figure(0), figure(0), figure(1)],
        ..Default::default()
    }
}

fn committed(outcome: Outcome) {
    assert!(matches!(outcome, Outcome::Committed(_)), "{outcome:?}");
}

#[test]
fn a_gate_refuses_a_submitted_activation_not_only_a_listed_one() {
    let rt = build(true);
    let mut engine = Engine::new(&rt, &NoHooks, state());
    let p0 = PlayerId(0);
    // Unit 0 is gated off; the command is refused and nothing changes.
    let refused = engine.submit(p0, Command::ActivateUnit(UnitId(0)));
    assert!(matches!(refused, Outcome::Rejected(_)), "{refused:?}");
    assert!(engine.state().flow.activation.is_none());
    assert!(engine.log().is_empty());
    committed(engine.submit(p0, Command::ActivateUnit(UnitId(1))));
}

#[test]
fn without_a_gate_every_own_unit_may_be_activated() {
    let rt = build(false);
    let mut engine = Engine::new(&rt, &NoHooks, state());
    committed(engine.submit(PlayerId(0), Command::ActivateUnit(UnitId(0))));
}
