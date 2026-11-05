use std::sync::Arc;

use strata_kernel::{DefId, Rotation, Space, Sym, Value};
use strata_model::{
    DieKind, FigureId, FigureState, FigureStatus, GameState, MatchSetup, PlayerId, PlayerRange,
    PlayerSetup, ScenarioDef, ScenarioSlot, UnitId, UnitState,
};
use strata_rules::{LogicError, NativeEnv};

use super::{PlaceUnit, deploying, placement_event};

struct Env(ScenarioDef);

impl NativeEnv for Env {
    fn stat(&self, _: &GameState, _: FigureId, _: &str) -> Result<i64, LogicError> {
        Err(LogicError("no stats".into()))
    }

    fn die(&self, _: &str) -> Option<(DefId, DieKind)> {
        None
    }

    fn scenario(&self) -> Option<&ScenarioDef> {
        Some(&self.0)
    }
}

fn slot(id: &str, deploy_order: u8) -> ScenarioSlot {
    ScenarioSlot {
        id: Sym::new(id),
        start: Sym::new("zone"),
        deploy_order,
    }
}

/// Slot `a` deploys second and slot `b` first. Player 0 sits in `a`, player 1 in `b`; each owns
/// one unit of one figure, in reserve.
fn setup() -> (Env, GameState) {
    let scenario = ScenarioDef {
        id: DefId(0),
        name: Default::default(),
        map: Sym::new("map"),
        players: PlayerRange { min: 2, max: 2 },
        slots: vec![slot("a", 2), slot("b", 1)],
        army: Vec::new(),
        relics: Vec::new(),
        victory: Vec::new(),
        addons: Vec::new(),
        after: Default::default(),
        tags: Default::default(),
        meta: Default::default(),
        source: Default::default(),
    };
    let seat = |id: u8, slot: &str| PlayerSetup {
        id: PlayerId(id),
        slot: Sym::new(slot),
        ..PlayerSetup::default()
    };
    let mut state = GameState {
        setup: Arc::new(MatchSetup {
            players: vec![seat(0, "a"), seat(1, "b")],
            ..MatchSetup::default()
        }),
        ..GameState::default()
    };
    for owner in 0..2u8 {
        state.units.push(UnitState {
            owner: PlayerId(owner),
            figures: vec![FigureId(u32::from(owner))],
            ..UnitState::default()
        });
        state.figures.push(FigureState {
            unit: UnitId(u32::from(owner)),
            controller: PlayerId(owner),
            ..FigureState::default()
        });
    }
    (Env(scenario), state)
}

#[test]
fn the_earliest_deploy_order_with_figures_in_reserve_deploys() {
    let (env, mut state) = setup();
    assert_eq!(deploying(&state, &env), Some(PlayerId(1)));
    state.figures[1].status = FigureStatus::OnBoard;
    assert_eq!(deploying(&state, &env), Some(PlayerId(0)));
    state.figures[0].status = FigureStatus::OnBoard;
    assert_eq!(deploying(&state, &env), None);
}

#[test]
fn a_player_without_a_slot_never_deploys() {
    let (env, mut state) = setup();
    Arc::make_mut(&mut state.setup).players.truncate(1);
    assert_eq!(deploying(&state, &env), Some(PlayerId(0)));
}

#[test]
fn a_placement_survives_its_command_payload() {
    let place = PlaceUnit {
        unit: UnitId(3),
        at: Space::new(-2, 5, 1),
        rotation: Rotation::new(4).unwrap(),
    };
    let strata_model::Command::Custom { id, payload } = place.command() else {
        panic!("place_unit is a custom command");
    };
    assert_eq!(id.as_str(), super::PLACE_UNIT);
    assert_eq!(PlaceUnit::parse(&payload), Ok(place));
}

#[test]
fn a_bad_payload_is_refused() {
    let place = PlaceUnit {
        unit: UnitId(0),
        at: Space::new(0, 0, 1),
        rotation: Rotation::default(),
    };
    let strata_model::Command::Custom { payload, .. } = place.command() else {
        panic!("place_unit is a custom command");
    };
    let Value::Map(mut fields) = payload else {
        panic!("the payload is a map");
    };
    fields.insert(Sym::new("rotation"), Value::Int(6));
    let err = PlaceUnit::parse(&Value::Map(fields.clone())).unwrap_err();
    assert!(err.contains("rotation"), "{err}");
    fields.remove("z");
    let err = PlaceUnit::parse(&Value::Map(fields)).unwrap_err();
    assert!(err.contains(" z"), "{err}");
}

#[test]
fn nothing_is_placed_once_the_first_round_has_begun() {
    let (env, mut state) = setup();
    let place = PlaceUnit {
        unit: UnitId(1),
        at: Space::new(0, 0, 1),
        rotation: Rotation::default(),
    };
    state.flow.round = 1;
    let err = placement_event(&state, &env, PlayerId(1), &place).unwrap_err();
    assert!(err.contains("deployment is over"), "{err}");
}

#[test]
fn a_placement_needs_a_start_zone_and_a_board() {
    let (env, state) = setup();
    let place = PlaceUnit {
        unit: UnitId(1),
        at: Space::new(0, 0, 1),
        rotation: Rotation::default(),
    };
    let err = placement_event(&state, &env, PlayerId(1), &place).unwrap_err();
    assert!(err.contains("no zone `zone`"), "{err}");
}
