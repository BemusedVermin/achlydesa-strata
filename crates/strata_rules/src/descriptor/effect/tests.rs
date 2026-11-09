use std::cell::RefCell;
use std::collections::BTreeMap;

use strata_kernel::{CommandRng, DefId, Hex, Space, Sym, Value, WindowId};
use strata_model::{
    AbilityDef, Answer, Decision, Descriptor, Event, FigureId, GameState, Logic, PlayerId,
    ScriptRef,
};

use super::super::{
    Base, Eff, FieldRole, Sel, Val, World, check_effect, check_logic, compile, native_effect,
};
use super::{EffectCx, EffectHost, run_effect};
use crate::check_ability;
use crate::engine::{Ctx, Halt};
use crate::native::{NativeEnv, NoEnv};
use crate::query::LogicError;

fn s(text: &str) -> Value {
    Value::Str(Sym::new(text))
}

fn eff(name: &str, rest: &[(&str, Value)]) -> Value {
    let mut map = BTreeMap::from([(Sym::new("eff"), s(name))]);
    map.extend(rest.iter().map(|(k, v)| (Sym::new(k), v.clone())));
    Value::Map(map)
}

fn compiled(value: Value) -> Eff {
    match compile(&Descriptor(value)).unwrap() {
        super::super::Node::Eff(e) => e,
        other => panic!("not an effect: {other:?}"),
    }
}

fn enemies() -> Value {
    Value::Map(BTreeMap::from([(Sym::new("sel"), s("enemies"))]))
}

fn enemy_sel() -> Sel {
    Sel {
        base: Base::Enemies,
        ..Sel::own()
    }
}

#[test]
fn every_effect_compiles_to_its_typed_form_with_defaults() {
    let cases = [
        (
            eff("wound", &[]),
            Eff::Wound {
                who: Sel::target(),
                amount: Val::Lit(1),
            },
        ),
        (
            eff("wound", &[("amount", Value::Int(2)), ("who", enemies())]),
            Eff::Wound {
                who: enemy_sel(),
                amount: Val::Lit(2),
            },
        ),
        (
            eff("heal", &[]),
            Eff::Heal {
                who: Sel::target(),
                amount: Val::Lit(1),
            },
        ),
        (
            eff("place", &[("within", Value::Int(2))]),
            Eff::Place {
                who: Sel::target(),
                within: Val::Lit(2),
                of: Sel::own(),
            },
        ),
        (eff("remove", &[]), Eff::Remove { who: Sel::target() }),
        (
            eff("move", &[("up_to", Value::Int(3))]),
            Eff::Move {
                who: Sel::target(),
                up_to: Val::Lit(3),
            },
        ),
        (
            eff("grant_slot", &[("slot", s("attack"))]),
            Eff::GrantSlot {
                slot: Sym::new("attack"),
                count: Val::Lit(1),
                who: Sel::own(),
            },
        ),
        (
            eff(
                "add_marker",
                &[("marker", s("stunned")), ("rounds", Value::Int(2))],
            ),
            Eff::AddMarker {
                marker: Sym::new("stunned"),
                count: Val::Lit(1),
                rounds: Some(2),
                who: Sel::target(),
            },
        ),
        (
            eff("take_control", &[]),
            Eff::TakeControl { who: Sel::target() },
        ),
        (eff("cancel_wounds", &[]), Eff::CancelWounds),
        (
            eff("excess_blocks_wound_attacker", &[]),
            Eff::ExcessBlocksWoundAttacker,
        ),
    ];
    for (value, want) in cases {
        assert_eq!(compiled(value), want);
    }
}

#[test]
fn malformed_effects_fail_to_compile() {
    let bad = [
        (eff("smite", &[]), "unknown descriptor `eff.smite`"),
        (
            eff("wound", &[("amonut", Value::Int(1))]),
            "`eff.wound` has no field `amonut`",
        ),
        (eff("place", &[]), "`eff.place` needs `within`"),
        (eff("move", &[]), "`eff.move` needs `up_to`"),
        (eff("grant_slot", &[]), "`eff.grant_slot` needs `slot`"),
        (eff("add_marker", &[]), "`eff.add_marker` needs `marker`"),
        (
            eff(
                "add_marker",
                &[("marker", s("x")), ("rounds", Value::Int(0))],
            ),
            "`eff.add_marker.rounds` must be at least 1",
        ),
        (
            eff("cancel_wounds", &[("now", Value::Bool(true))]),
            "has no field `now`",
        ),
    ];
    for (value, message) in bad {
        let err = compile(&Descriptor(value)).unwrap_err().to_string();
        assert!(err.contains(message), "{err} lacks {message}");
    }
}

fn native(value: Value) -> Logic {
    Logic::Native(Descriptor(value))
}

fn script() -> Logic {
    Logic::Script(ScriptRef {
        pack: strata_kernel::PackId::new("t"),
        def: DefId(0),
        path: Sym::new("x"),
    })
}

#[test]
fn native_effect_finds_only_eff_descriptors() {
    assert!(native_effect(&native(eff("remove", &[]))).unwrap().is_ok());
    assert!(native_effect(&native(eff("nope", &[]))).unwrap().is_err());
    assert!(native_effect(&native(enemies())).is_none());
    let named = Value::Map(BTreeMap::from([(Sym::new("native"), s("core:x"))]));
    assert!(native_effect(&native(named.clone())).is_none());
    assert!(native_effect(&script()).is_none());
    // A logic that acts must be an effect, Rust logic or a script.
    assert!(check_effect(&native(named.clone()), true).is_ok());
    assert!(check_effect(&script(), true).is_ok());
    assert!(check_effect(&native(eff("heal", &[])), true).is_ok());
    let not_an_effect = check_effect(&native(enemies()), true).unwrap_err();
    assert!(not_an_effect.to_string().contains("must be an `eff`"));
    assert!(check_logic(&native(enemies()), FieldRole::Asks).is_ok());
    assert!(check_logic(&native(eff("heal", &[])), FieldRole::Acts).is_ok());
    // Rust logic by name runs only as a trigger's effect or an action's resolver.
    let named = native(named);
    assert!(check_logic(&named, FieldRole::ActsOrNamesRust).is_ok());
    let refused = check_logic(&named, FieldRole::Acts).unwrap_err();
    assert!(refused.to_string().contains("runs only as"), "{refused}");
    assert!(check_effect(&named, false).is_err());
    assert_eq!(
        FieldRole::of("triggers", "effect"),
        FieldRole::ActsOrNamesRust
    );
    assert_eq!(
        FieldRole::of("action", "resolve"),
        FieldRole::ActsOrNamesRust
    );
    assert_eq!(FieldRole::of("reactions", "effect"), FieldRole::Acts);
    assert_eq!(FieldRole::of("replacement", "replace"), FieldRole::Acts);
    assert_eq!(FieldRole::of("triggers", "filter"), FieldRole::Asks);
}

#[test]
fn check_ability_reports_the_route_to_a_bad_effect() {
    let ability: AbilityDef = serde_json::from_value(serde_json::json!({
        "name": "Test", "text": "Test.", "scope": "figure",
        "triggers": [
            { "event": "attack", "effect": { "native": { "Map": { "native": { "Str": "core:x" } } } } },
            { "event": "attack", "effect": { "native": { "Map": { "eff": { "Str": "heel" } } } } },
        ],
    }))
    .unwrap();
    let fault = check_ability(&ability).unwrap_err();
    assert_eq!(fault.path, "triggers/2/effect");
    assert_eq!(fault.error.to_string(), "unknown descriptor `eff.heel`");
}

#[test]
fn check_ability_refuses_rust_logic_by_name_where_it_cannot_run() {
    let ability: AbilityDef = serde_json::from_value(serde_json::json!({
        "name": "Test", "text": "Test.", "scope": "figure",
        "reactions": [
            { "window": "w", "effect": { "native": { "Map": { "native": { "Str": "core:x" } } } } },
        ],
    }))
    .unwrap();
    let fault = check_ability(&ability).unwrap_err();
    assert_eq!(fault.path, "reactions/1/effect");
    assert!(fault.error.to_string().contains("runs only as"), "{fault}");
}

/// A world of four figures in a row on level 1; 0 and 1 belong to player 0.
struct Row;

impl World for Row {
    fn figures(&self) -> Vec<FigureId> {
        (0..4).map(FigureId).collect()
    }
    fn location(&self, f: FigureId) -> Option<Space> {
        Some(Space::new(f.0 as i32, 0, 1))
    }
    fn controller(&self, f: FigureId) -> PlayerId {
        PlayerId(u8::from(f.0 >= 2))
    }
    fn team(&self, p: PlayerId) -> u8 {
        p.0
    }
    fn has_trait(&self, _: FigureId, _: &str) -> bool {
        false
    }
    fn kind(&self, _: FigureId) -> Sym {
        Sym::new("squad")
    }
    fn stat(&self, _: FigureId, _: &str) -> Result<i64, LogicError> {
        Ok(2)
    }
    fn terrain_at(&self, _: Space) -> Option<Sym> {
        None
    }
    fn occupant(&self, _: Space) -> Option<FigureId> {
        None
    }
    fn phase(&self) -> Sym {
        Sym::new("turns")
    }
    fn levels(&self, _: Hex) -> Vec<u32> {
        vec![1]
    }
}

/// Records the module calls an effect makes; `combat.current` answers `attack`.
struct Host {
    attack: Value,
    calls: RefCell<Vec<String>>,
}

impl EffectHost for Host {
    fn env(&self) -> &dyn NativeEnv {
        &NoEnv
    }
    fn world<'a>(&'a self, _: &'a GameState) -> Result<Box<dyn World + 'a>, LogicError> {
        Ok(Box::new(Row))
    }
    fn api(&self, _: &mut dyn Ctx, ns: &str, name: &str, args: &[Value]) -> Result<Value, Halt> {
        self.calls
            .borrow_mut()
            .push(format!("{ns}.{name} {args:?}"));
        Ok(if name == "current" {
            self.attack.clone()
        } else {
            Value::Nil
        })
    }
}

struct Bench {
    state: GameState,
    rng: CommandRng,
}

impl Ctx for Bench {
    fn state(&self) -> &GameState {
        &self.state
    }
    fn rng(&mut self) -> &mut CommandRng {
        &mut self.rng
    }
    fn emit(&mut self, _: Event) -> Result<(), Halt> {
        Err(Halt::reject("the bench applies nothing"))
    }
    fn window(&mut self, _: &WindowId) -> Result<(), Halt> {
        Ok(())
    }
    fn choose(&mut self, _: PlayerId, _: Decision) -> Result<Answer, Halt> {
        Err(Halt::reject("the bench asks nothing"))
    }
}

fn attack(hits: i64, blocks: i64) -> Value {
    let fields = [("attacker", 0), ("hits", hits), ("blocks", blocks)];
    Value::Map(
        fields
            .into_iter()
            .map(|(k, v)| (Sym::new(k), Value::Int(v)))
            .collect(),
    )
}

fn calls(e: &Eff, host: &Host, cx: &EffectCx<'_>) -> Result<Vec<String>, Halt> {
    let mut bench = Bench {
        state: GameState::default(),
        rng: CommandRng::for_command(0, 0),
    };
    run_effect(e, &mut bench, host, cx)?;
    Ok(host.calls.take())
}

fn host(attack: Value) -> Host {
    Host {
        attack,
        calls: RefCell::default(),
    }
}

const NOBODY: EffectCx<'static> = EffectCx {
    subject: Value::Nil,
    target: None,
    replacing: None,
};

#[test]
fn cancel_wounds_cancels_the_attack_leaving_hits_and_blocks_alone() {
    let done = calls(&Eff::CancelWounds, &host(attack(3, 1)), &NOBODY).unwrap();
    assert_eq!(done.len(), 2);
    assert_eq!(done[1], "combat.cancel []", "{done:?}");
}

#[test]
fn cancel_wounds_needs_an_attack_unless_it_replaces_wounds() {
    let err = calls(&Eff::CancelWounds, &host(Value::Nil), &NOBODY).unwrap_err();
    assert!(format!("{err:?}").contains("needs an attack in progress"));
    let wounds = Event::WoundsApplied {
        figure: FigureId(1),
        wound: DefId(1),
        amount: 2,
    };
    let replacing = EffectCx {
        replacing: Some(&wounds),
        ..NOBODY
    };
    assert_eq!(
        calls(&Eff::CancelWounds, &host(Value::Nil), &replacing).unwrap(),
        Vec::<String>::new()
    );
}

#[test]
fn excess_blocks_skip_an_attacker_out_of_play() {
    // The bench state lists no figures, so the attacker is out of play: only `current` is
    // called. The fixture pack tests wound a live attacker.
    let e = Eff::ExcessBlocksWoundAttacker;
    assert_eq!(calls(&e, &host(attack(1, 3)), &NOBODY).unwrap().len(), 1);
    let err = calls(&e, &host(Value::Nil), &NOBODY).unwrap_err();
    assert!(format!("{err:?}").contains("needs an attack in progress"));
}

#[test]
fn a_negative_amount_is_refused() {
    let e = Eff::Wound {
        who: Sel::target(),
        amount: Val::Lit(-1),
    };
    let err = calls(&e, &host(Value::Nil), &NOBODY).unwrap_err();
    assert!(format!("{err:?}").contains("must not be negative"));
}

#[test]
fn a_zero_amount_does_nothing() {
    let e = Eff::Wound {
        who: Sel::target(),
        amount: Val::Lit(0),
    };
    let cx = EffectCx {
        target: Some(FigureId(2)),
        ..NOBODY
    };
    assert!(calls(&e, &host(Value::Nil), &cx).unwrap().is_empty());
}

#[test]
fn the_event_context_aims_at_the_figure_the_event_is_about() {
    let state = GameState::default();
    let cx = EffectCx::for_event(&state, &Event::FigureDestroyed(FigureId(3)));
    assert_eq!(cx.subject, Value::Int(3));
    assert_eq!(cx.target, Some(FigureId(3)));
    let none = EffectCx::for_event(&state, &Event::ActivationEnded);
    assert_eq!(none, NOBODY);
}
