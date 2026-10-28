//! `GameState::apply`: one test per event kind, error cases, and hash behaviour.

use strata_kernel::{DefId, ModuleId, Rotation, Space, Sym, Value};
use strata_model::*;

fn base() -> GameState {
    GameState {
        players: vec![
            PlayerState {
                team: 0,
                hand: vec![DefId(2), DefId(3)],
            },
            PlayerState::default(),
        ],
        units: vec![UnitState {
            figures: vec![FigureId(0), FigureId(1)],
            ..Default::default()
        }],
        figures: vec![FigureState::default(), FigureState::default()],
        relics: vec![RelicState {
            location: Some(Space::new(2, 0, 0)),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn mv(figure: u32, q: i32) -> Event {
    Event::FigureMoved {
        figure: FigureId(figure),
        to: Space::new(q, 0, 0),
        rotation: Rotation::from_steps(2),
    }
}

/// Applies `event`, asserting that the hash changes.
fn applied(state: &mut GameState, event: &Event) {
    let before = state.hash();
    state.apply(event).expect("event is valid");
    assert_ne!(state.hash(), before, "{event:?} should change the state");
}

/// Applies `event`, asserting that it fails and leaves the state untouched.
fn rejected(state: &GameState, event: &Event) -> ApplyError {
    let mut copy = state.clone();
    let err = copy.apply(event).expect_err("event is invalid");
    assert_eq!(copy, *state, "{event:?} must not partially apply");
    err
}

fn wound(figure: u32, amount: u32) -> Event {
    Event::WoundsApplied {
        figure: FigureId(figure),
        wound: DefId(9),
        amount,
    }
}

fn marker(figure: u32, delta: i32) -> Event {
    Event::MarkerChanged {
        figure: FigureId(figure),
        marker: DefId(8),
        delta,
        source: Some(FigureId(1)),
        expiry: Some(3),
    }
}

#[test]
fn figure_moved_places_and_relocates() {
    let mut s = base();
    applied(&mut s, &mv(0, 1));
    let fig = &s.figures[0];
    assert_eq!(fig.location, Some(Space::new(1, 0, 0)));
    assert_eq!(fig.rotation, Rotation::from_steps(2));
    assert_eq!(fig.status, FigureStatus::OnBoard);
    assert_eq!(s.board_changes.occupancy.len(), 1);

    applied(&mut s, &mv(0, 2));
    assert_eq!(s.board_changes.occupancy.len(), 1);
    assert_eq!(
        s.board_changes.occupancy.get(&Space::new(2, 0, 0)),
        Some(&FigureId(0))
    );
}

#[test]
fn figure_moved_rejects_bad_targets() {
    let mut s = base();
    applied(&mut s, &mv(0, 1));
    assert_eq!(rejected(&s, &mv(1, 1)), ApplyError::Occupied(FigureId(0)));
    assert_eq!(rejected(&s, &mv(5, 1)), ApplyError::NoFigure(FigureId(5)));
    applied(&mut s, &Event::FigureDestroyed(FigureId(0)));
    assert_eq!(
        rejected(&s, &mv(0, 3)),
        ApplyError::WrongStatus(FigureId(0), FigureStatus::Destroyed)
    );
}

#[test]
fn dice_rolled_and_custom_change_nothing() {
    let mut s = base();
    let before = s.hash();
    s.apply(&Event::DiceRolled {
        die: DefId(1),
        results: vec![3, 4],
    })
    .unwrap();
    s.apply(&Event::Custom {
        id: Sym::new("flash"),
        payload: Value::from(1),
    })
    .unwrap();
    assert_eq!(s.hash(), before);
}

#[test]
fn wounds_stack_on_one_marker() {
    let mut s = base();
    applied(&mut s, &wound(0, 2));
    applied(&mut s, &wound(0, 1));
    let markers = &s.figures[0].markers;
    assert_eq!(markers.len(), 1);
    assert_eq!((markers[0].def, markers[0].count), (DefId(9), 3));
    assert_eq!(
        rejected(&s, &wound(0, 0)),
        ApplyError::MarkerCount(FigureId(0))
    );
    assert_eq!(
        rejected(&s, &wound(4, 1)),
        ApplyError::NoFigure(FigureId(4))
    );
}

#[test]
fn marker_changed_adds_reduces_and_removes() {
    let mut s = base();
    applied(&mut s, &marker(0, 2));
    let m = &s.figures[0].markers[0];
    assert_eq!(
        (m.count, m.source, m.expiry),
        (2, Some(FigureId(1)), Some(3))
    );
    applied(&mut s, &marker(0, -1));
    assert_eq!(s.figures[0].markers[0].count, 1);
    applied(&mut s, &marker(0, -1));
    assert!(s.figures[0].markers.is_empty());
}

#[test]
fn marker_changed_rejects_underflow_and_zero() {
    let mut s = base();
    assert_eq!(
        rejected(&s, &marker(0, -1)),
        ApplyError::MarkerCount(FigureId(0))
    );
    assert_eq!(
        rejected(&s, &marker(0, 0)),
        ApplyError::MarkerCount(FigureId(0))
    );
    applied(&mut s, &marker(0, 1));
    let other_source = Event::MarkerChanged {
        figure: FigureId(0),
        marker: DefId(8),
        delta: -1,
        source: None,
        expiry: Some(3),
    };
    assert_eq!(
        rejected(&s, &other_source),
        ApplyError::MarkerCount(FigureId(0))
    );
}

#[test]
fn figure_destroyed_leaves_the_board() {
    let mut s = base();
    applied(&mut s, &mv(0, 1));
    applied(&mut s, &Event::FigureDestroyed(FigureId(0)));
    assert_eq!(s.figures[0].status, FigureStatus::Destroyed);
    assert_eq!(s.figures[0].location, None);
    assert!(s.board_changes.occupancy.is_empty());
    assert_eq!(
        rejected(&s, &Event::FigureDestroyed(FigureId(0))),
        ApplyError::WrongStatus(FigureId(0), FigureStatus::Destroyed)
    );
}

fn place(kind: u32, visibility: Visibility) -> Event {
    Event::TokenPlaced {
        unit: UnitId(0),
        token: Token {
            kind: DefId(kind),
            visibility,
        },
    }
}

#[test]
fn token_placed_moves_from_hand_to_card() {
    let mut s = base();
    applied(&mut s, &place(3, Visibility::Hidden));
    assert_eq!(s.players[0].hand, vec![DefId(2)]);
    assert_eq!(s.units[0].tokens.len(), 1);
    assert_eq!(
        rejected(&s, &place(3, Visibility::Public)),
        ApplyError::TokenNotInHand(DefId(3))
    );
    let missing = Event::TokenPlaced {
        unit: UnitId(4),
        token: s.units[0].tokens[0].clone(),
    };
    assert_eq!(rejected(&s, &missing), ApplyError::NoUnit(UnitId(4)));
}

#[test]
fn token_revealed_turns_a_hidden_token_face_up() {
    let mut s = base();
    applied(&mut s, &place(2, Visibility::Hidden));
    let reveal = Event::TokenRevealed {
        unit: UnitId(0),
        index: 0,
    };
    applied(&mut s, &reveal);
    assert_eq!(s.units[0].tokens[0].visibility, Visibility::Public);
    assert_eq!(
        rejected(&s, &reveal),
        ApplyError::NoHiddenToken(UnitId(0), 0)
    );
    let out_of_range = Event::TokenRevealed {
        unit: UnitId(0),
        index: 7,
    };
    assert_eq!(
        rejected(&s, &out_of_range),
        ApplyError::NoHiddenToken(UnitId(0), 7)
    );
}

#[test]
fn phases_push_and_pop_in_order() {
    let mut s = base();
    applied(&mut s, &Event::PhaseEntered(DefId(1)));
    applied(&mut s, &Event::PhaseEntered(DefId(2)));
    assert_eq!(
        rejected(&s, &Event::PhaseExited(DefId(1))),
        ApplyError::PhaseMismatch(DefId(1))
    );
    applied(&mut s, &Event::PhaseExited(DefId(2)));
    applied(&mut s, &Event::PhaseExited(DefId(1)));
    assert!(s.flow.phases.is_empty());
    assert_eq!(
        rejected(&s, &Event::PhaseExited(DefId(1))),
        ApplyError::PhaseMismatch(DefId(1))
    );
}

#[test]
fn relic_claimed_sets_holder_once() {
    let mut s = base();
    let claim = |by| Event::RelicClaimed {
        relic: RelicId(0),
        by: PlayerId(by),
    };
    applied(&mut s, &claim(1));
    assert_eq!(s.relics[0].holder, Some(PlayerId(1)));
    assert_eq!(s.relics[0].location, None);
    assert_eq!(rejected(&s, &claim(0)), ApplyError::RelicHeld(RelicId(0)));
    assert_eq!(
        rejected(&base(), &claim(9)),
        ApplyError::NoPlayer(PlayerId(9))
    );
    let missing = Event::RelicClaimed {
        relic: RelicId(3),
        by: PlayerId(0),
    };
    assert_eq!(rejected(&base(), &missing), ApplyError::NoRelic(RelicId(3)));
}

#[test]
fn var_set_inserts_overwrites_and_removes() {
    let mut s = base();
    let key = VarKey {
        module: ModuleId::new("m"),
        name: Sym::new("x"),
    };
    let set = |value: Value| Event::VarSet {
        key: key.clone(),
        value,
    };
    applied(&mut s, &set(Value::from(1)));
    applied(&mut s, &set(Value::from(2)));
    assert_eq!(s.vars.get(&key), Some(&Value::from(2)));
    applied(&mut s, &set(Value::Nil));
    assert!(s.vars.is_empty());
    assert_eq!(s.hash(), base().hash());
}

#[test]
fn control_changed_sets_controller() {
    let mut s = base();
    let change = |figure, to| Event::ControlChanged {
        figure: FigureId(figure),
        controller: PlayerId(to),
    };
    applied(&mut s, &change(1, 1));
    assert_eq!(s.figures[1].controller, PlayerId(1));
    assert_eq!(
        rejected(&s, &change(1, 5)),
        ApplyError::NoPlayer(PlayerId(5))
    );
    assert_eq!(
        rejected(&s, &change(5, 1)),
        ApplyError::NoFigure(FigureId(5))
    );
}

#[test]
fn applying_to_a_clone_leaves_the_original_untouched() {
    let original = base();
    let before = original.hash();
    let mut copy = original.clone();
    for event in [
        mv(0, 1),
        wound(0, 1),
        place(2, Visibility::Public),
        Event::PhaseEntered(DefId(1)),
        Event::RelicClaimed {
            relic: RelicId(0),
            by: PlayerId(0),
        },
    ] {
        copy.apply(&event).unwrap();
    }
    assert_ne!(copy.hash(), before);
    assert_eq!(original.hash(), before);
    assert_eq!(original, base());
}

#[test]
fn same_events_give_the_same_hash_sequence() {
    let events = [
        mv(0, 1),
        mv(1, 2),
        wound(0, 2),
        Event::FigureDestroyed(FigureId(1)),
        Event::PhaseEntered(DefId(4)),
    ];
    let run = || {
        let mut s = base();
        events
            .iter()
            .map(|e| {
                s.apply(e).unwrap();
                s.hash()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
}
