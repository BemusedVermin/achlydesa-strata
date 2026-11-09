use proptest::prelude::*;
use strata_kernel::{DefId, Rotation, Space, Sym, Value};
use strata_model::{
    DieKind, FigureId, FigureState, FigureStatus, GameState, PlayerId, PlayerState, UnitId,
    UnitState,
};
use strata_rules::{DeploySlot, LogicError, NativeEnv};

use super::{PlaceUnit, Spot, cursor_key, deploying};

fn payload(spots: Vec<Vec<i64>>) -> Value {
    let at = spots
        .into_iter()
        .map(|s| Value::List(s.into_iter().map(Value::Int).collect()))
        .collect();
    let entries = [
        (Sym::new("unit"), Value::Int(3)),
        (Sym::new("at"), Value::List(at)),
    ];
    Value::Map(entries.into())
}

#[test]
fn a_missing_rotation_means_the_default_facing() {
    let put = PlaceUnit::from_value(&payload(vec![vec![1, -2, 3]])).unwrap();
    let spot = Spot {
        anchor: Space::new(1, -2, 3),
        rotation: Rotation::default(),
    };
    assert_eq!(
        put,
        PlaceUnit {
            unit: UnitId(3),
            at: vec![spot]
        }
    );
}

#[test]
fn a_rotation_is_read_from_the_fourth_place() {
    let put = PlaceUnit::from_value(&payload(vec![vec![0, 0, 1, 4]])).unwrap();
    assert_eq!(put.at[0].rotation, Rotation::new(4).unwrap());
}

#[test]
fn a_rotation_outside_zero_to_five_is_refused() {
    for o in [-1, 6] {
        let err = PlaceUnit::from_value(&payload(vec![vec![0, 0, 1, o]])).unwrap_err();
        assert!(err.contains("rotation"), "{err}");
    }
}

#[test]
fn a_spot_of_the_wrong_shape_is_refused() {
    for spot in [vec![0, 0], vec![0, 0, 1, 0, 0]] {
        let err = PlaceUnit::from_value(&payload(vec![spot])).unwrap_err();
        assert!(err.contains("each spot"), "{err}");
    }
    let err = PlaceUnit::from_value(&payload(vec![vec![0, 0, -1]])).unwrap_err();
    assert!(err.contains("negative"), "{err}");
}

proptest! {
    #[test]
    fn a_payload_reads_back_what_it_wrote(
        unit in 0u32..1000,
        spots in prop::collection::vec((-50i32..50, -50i32..50, 0u32..20, 0u8..6), 0..4),
    ) {
        let at = spots.into_iter().map(|(q, r, z, o)| Spot {
            anchor: Space::new(q, r, z),
            rotation: Rotation::new(o).unwrap(),
        });
        let put = PlaceUnit { unit: UnitId(unit), at: at.collect() };
        prop_assert_eq!(PlaceUnit::from_value(&put.to_value()), Ok(put));
    }
}

/// Gives each player the start-zone slot at its index in `orders`; `None` means no start zone.
struct Slots(Vec<Option<u8>>);

impl NativeEnv for Slots {
    fn stat(&self, _: &GameState, _: FigureId, stat: &str) -> Result<i64, LogicError> {
        Err(LogicError(format!("no stat `{stat}`")))
    }
    fn die(&self, _: &str) -> Option<(DefId, DieKind)> {
        None
    }
    fn deployment(&self, _: &GameState, player: PlayerId) -> Option<DeploySlot> {
        let order = (*self.0.get(usize::from(player.0))?)?;
        let zone = Vec::new();
        Some(DeploySlot { order, zone })
    }
}

/// One one-figure unit in reserve per entry of `owners`, owned by that player, among `players`.
fn reserves(players: usize, owners: &[u8]) -> GameState {
    let figure = FigureState {
        status: FigureStatus::Reserve,
        ..FigureState::default()
    };
    let units = (0u32..).zip(owners).map(|(i, owner)| UnitState {
        owner: PlayerId(*owner),
        figures: vec![FigureId(i)],
        ..UnitState::default()
    });
    GameState {
        players: vec![PlayerState::default(); players],
        units: units.collect(),
        figures: vec![figure; owners.len()],
        ..GameState::default()
    }
}

#[test]
fn the_deploying_player_follows_slot_order_and_the_cursor() {
    // Player 1's slot places first.
    let env = Slots(vec![Some(1), Some(0)]);
    let mut state = reserves(2, &[0, 1, 1]);
    assert_eq!(deploying(&state, &env), Some(PlayerId(1)));
    state.vars.insert(cursor_key(), Value::Int(1));
    assert_eq!(deploying(&state, &env), Some(PlayerId(0)));
    // Player 0 has placed everything, so the turn passes back to player 1.
    state.figures[0].status = FigureStatus::OnBoard;
    assert_eq!(deploying(&state, &env), Some(PlayerId(1)));
    state.figures[1].status = FigureStatus::OnBoard;
    state.figures[2].status = FigureStatus::OnBoard;
    assert_eq!(deploying(&state, &env), None, "nothing is left to place");
}

#[test]
fn nobody_deploys_once_the_game_has_started_or_without_a_start_zone() {
    let mut state = reserves(2, &[0, 1]);
    assert_eq!(deploying(&state, &Slots(vec![Some(0), None])), None);
    let env = Slots(vec![Some(0), Some(1)]);
    assert_eq!(deploying(&state, &env), Some(PlayerId(0)));
    state.flow.round = 1;
    assert_eq!(deploying(&state, &env), None);
}
