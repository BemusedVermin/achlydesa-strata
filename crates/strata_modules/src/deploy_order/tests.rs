use strata_kernel::DefId;
use strata_model::{DieKind, FigureId, FigureState, PlayerState, UnitState};
use strata_rules::{DeploySlot, LogicError};

use super::*;

/// Gives each player a start zone with the `deploy_order` at their index.
struct Slots(Vec<u8>);

impl NativeEnv for Slots {
    fn stat(&self, _: &GameState, _: FigureId, stat: &str) -> Result<i64, LogicError> {
        Err(LogicError(format!("no stat `{stat}`")))
    }
    fn die(&self, _: &str) -> Option<(DefId, DieKind)> {
        None
    }
    fn deployment(&self, _: &GameState, player: PlayerId) -> Option<DeploySlot> {
        let order = *self.0.get(player.index())?;
        Some(DeploySlot {
            order,
            zone: Vec::new(),
        })
    }
}

/// One single-figure unit per entry, owned by that player and waiting in reserve.
fn state(owners: &[u8]) -> GameState {
    let unit = |(i, owner): (usize, &u8)| UnitState {
        owner: PlayerId(*owner),
        figures: vec![FigureId(i as u32)],
        ..Default::default()
    };
    let figure = |(i, _): (usize, &u8)| FigureState {
        unit: strata_model::UnitId(i as u32),
        ..Default::default()
    };
    GameState {
        players: vec![PlayerState::default(); 2],
        units: owners.iter().enumerate().map(unit).collect(),
        figures: owners.iter().enumerate().map(figure).collect(),
        ..Default::default()
    }
}

/// Places the first reserve unit of whoever `deploying` names, then moves the cursor the way
/// the driver does, until nobody is left; returns who placed, in order.
fn play(owners: &[u8], env: &Slots, order: Order) -> Vec<u8> {
    let mut state = state(owners);
    let seats = seating(&state, env).unwrap();
    let mut placed_by = Vec::new();
    while let Some(player) = deploying(&state, env, order) {
        let unit = state.units.iter().position(|u| {
            u.owner == player && state.figures[u.figures[0].index()].status == FigureStatus::Reserve
        });
        let figure = state.units[unit.unwrap()].figures[0];
        state.figures[figure.index()].status = FigureStatus::OnBoard;
        let seat = seats.iter().position(|p| *p == player).unwrap();
        let next = (seat + 1) % seats.len();
        state.vars.insert(cursor_key(), Value::Int(next as i64));
        placed_by.push(player.0);
    }
    placed_by
}

#[test]
fn alternate_order_takes_one_unit_from_each_seat_in_turn() {
    let got = play(&[0, 0, 0, 1, 1], &Slots(vec![1, 2]), Order::Alternate);
    assert_eq!(got, [0, 1, 0, 1, 0]);
}

#[test]
fn army_order_places_a_whole_reserve_before_the_next_seat() {
    let got = play(&[0, 0, 0, 1, 1], &Slots(vec![1, 2]), Order::Army);
    assert_eq!(got, [0, 0, 0, 1, 1]);
}

#[test]
fn the_lower_deploy_order_goes_first_in_either_order() {
    let slots = Slots(vec![2, 1]);
    assert_eq!(play(&[0, 1, 1], &slots, Order::Alternate), [1, 0, 1]);
    assert_eq!(play(&[0, 1, 1], &slots, Order::Army), [1, 1, 0]);
}

#[test]
fn a_tie_in_deploy_order_goes_to_the_lower_player() {
    assert_eq!(play(&[1, 0], &Slots(vec![1, 1]), Order::Alternate), [0, 1]);
}

#[test]
fn nobody_deploys_once_the_game_has_started_or_without_start_zones() {
    let mut started = state(&[0, 1]);
    started.flow.round = 1;
    assert_eq!(deploying(&started, &Slots(vec![1, 2]), Order::Army), None);
    assert!(anyone_waiting(&started, &Slots(vec![1, 2])));
    let unzoned = state(&[0, 1]);
    assert_eq!(deploying(&unzoned, &Slots(vec![1]), Order::Alternate), None);
    assert!(!anyone_waiting(&unzoned, &Slots(vec![1])));
}

#[test]
fn the_service_names_the_deploying_player() {
    let state = state(&[1, 0]);
    let turn = DeployTurn(Order::Army);
    assert_eq!(
        turn.next_actor(&state, &Slots(vec![2, 1])),
        Some(PlayerId(1))
    );
}

#[test]
fn only_the_two_documented_orders_are_named() {
    assert_eq!(Order::named("alternate"), Some(Order::Alternate));
    assert_eq!(Order::named("army"), Some(Order::Army));
    assert_eq!(Order::named("whole"), None);
}
