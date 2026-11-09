//! Progress estimates and result reading of `core:victory`; ending matches is covered by pack
//! tests.

use std::collections::BTreeMap;

use strata_kernel::{DefId, Frac, Sym, Value};
use strata_model::{
    Descriptor, DieKind, FigureId, FigureState, FigureStatus, GameState, Logic, PlayerId,
    PlayerRange, PlayerState, ScenarioDef, Text, UnitId, UnitState,
};
use strata_rules::{LogicError, NativeEnv, Victory};

use super::{Service, key};

/// Knows the scenario and nothing else.
struct Env(ScenarioDef);

impl NativeEnv for Env {
    fn stat(&self, _: &GameState, _: FigureId, stat: &str) -> Result<i64, LogicError> {
        Err(LogicError(format!("no stat `{stat}`")))
    }
    fn die(&self, _: &str) -> Option<(DefId, DieKind)> {
        None
    }
    fn scenario(&self, _: &GameState) -> Option<&ScenarioDef> {
        Some(&self.0)
    }
}

fn win(fields: &[(&str, Value)]) -> Logic {
    let map = fields.iter().map(|(k, v)| (Sym::new(k), v.clone()));
    Logic::Native(Descriptor(Value::Map(map.collect())))
}

fn text(s: &str) -> Value {
    Value::Str(Sym::new(s))
}

fn scenario() -> ScenarioDef {
    ScenarioDef {
        id: DefId(0),
        name: Text("s".into()),
        map: Sym::new("m"),
        players: PlayerRange { min: 2, max: 2 },
        slots: Vec::new(),
        army: Vec::new(),
        relics: Vec::new(),
        victory: vec![
            win(&[("win", text("eliminate_all"))]),
            win(&[
                ("win", text("hold_zone")),
                ("zone", text("z")),
                ("rounds", Value::Int(2)),
            ]),
            win(&[("win", text("round_limit")), ("rounds", Value::Int(3))]),
        ],
        addons: Vec::new(),
        after: BTreeMap::new(),
        tags: Default::default(),
        meta: BTreeMap::new(),
        source: Default::default(),
    }
}

/// Player 0 fields one figure; player 1 two, one of them destroyed. Player 0 has held the zone
/// at one round end.
fn state() -> GameState {
    let mut state = GameState::default();
    for team in 0..2 {
        state.players.push(PlayerState {
            team,
            hand: Vec::new(),
        });
    }
    let figure = |unit: u32, owner: u8, status| FigureState {
        unit: UnitId(unit),
        controller: PlayerId(owner),
        status,
        ..FigureState::default()
    };
    state.figures = vec![
        figure(0, 0, FigureStatus::OnBoard),
        figure(1, 1, FigureStatus::OnBoard),
        figure(1, 1, FigureStatus::Destroyed),
    ];
    state.units = vec![
        UnitState {
            owner: PlayerId(0),
            figures: vec![FigureId(0)],
            ..UnitState::default()
        },
        UnitState {
            owner: PlayerId(1),
            figures: vec![FigureId(1), FigureId(2)],
            ..UnitState::default()
        },
    ];
    let held = [
        (Sym::new("team"), Value::Int(0)),
        (Sym::new("rounds"), Value::Int(1)),
    ];
    state.vars.insert(key("held.1"), Value::Map(held.into()));
    state
}

#[test]
fn progress_estimates_elimination_and_holding_but_not_the_round_limit() {
    let env = Env(scenario());
    let found = Service.progress(&state(), &env, PlayerId(0)).unwrap();
    let got: Vec<(&str, usize, Frac)> = found
        .iter()
        .map(|p| (p.condition.as_str(), p.index, p.value))
        .collect();
    let half = Frac::new(1, 2);
    assert_eq!(got, [("eliminate_all", 0, half), ("hold_zone", 1, half)]);

    let found = Service.progress(&state(), &env, PlayerId(1)).unwrap();
    let got: Vec<Frac> = found.iter().map(|p| p.value).collect();
    assert_eq!(got, [Frac::ZERO, Frac::ZERO]);
}

#[test]
fn no_result_until_one_is_recorded() {
    let mut state = state();
    assert_eq!(Service.result(&state), None);
    let result = [
        (Sym::new("condition"), text("hold_zone")),
        (Sym::new("index"), Value::Int(1)),
        (Sym::new("winners"), Value::List(vec![Value::Int(0)])),
    ];
    state.vars.insert(key("result"), Value::Map(result.into()));
    let found = Service.result(&state).unwrap();
    assert_eq!((found.condition.as_str(), found.index), ("hold_zone", 1));
    assert_eq!(found.winners, [PlayerId(0)]);
}
