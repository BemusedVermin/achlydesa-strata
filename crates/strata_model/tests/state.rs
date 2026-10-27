//! State hashing, cloning and serde round trips for commands, events and decisions.

use std::sync::Arc;

use strata_kernel::{DefId, ModuleId, Rotation, Space, Sym, Value};
use strata_model::*;

fn base() -> GameState {
    GameState {
        players: vec![PlayerState {
            team: 0,
            hand: vec![DefId(2)],
        }],
        units: vec![UnitState {
            figures: vec![FigureId(0)],
            ..Default::default()
        }],
        figures: vec![FigureState::default()],
        relics: vec![RelicState::default()],
        ..Default::default()
    }
}

#[test]
fn hash_is_stable_and_changes_with_every_part_of_state() {
    let state = base();
    assert_eq!(state.hash(), base().hash());
    assert_eq!(state.hash(), state.clone().hash());

    let mut variants: Vec<GameState> = Vec::new();
    let mut edit = |f: &dyn Fn(&mut GameState)| {
        let mut s = base();
        f(&mut s);
        variants.push(s);
    };
    edit(&|s| s.command_index = 1);
    edit(&|s| s.players[0].hand.clear());
    edit(&|s| s.units[0].copy = 1);
    edit(&|s| s.figures[0].location = Some(Space::new(1, 0, 0)));
    edit(&|s| s.figures[0].rotation = Rotation::from_steps(1));
    edit(&|s| s.figures[0].status = FigureStatus::OnBoard);
    edit(&|s| s.figures[0].controller = PlayerId(1));
    edit(&|s| s.relics[0].face_down = true);
    edit(&|s| s.flow.round = 1);
    edit(&|s| s.flow.phases.push(PhaseFrame::default()));
    edit(&|s| {
        s.board_changes
            .occupancy
            .insert(Space::new(0, 0, 0), FigureId(0));
    });
    edit(&|s| {
        s.vars.insert(
            VarKey {
                module: ModuleId::new("m"),
                name: Sym::new("x"),
            },
            Value::from(1),
        );
    });
    edit(&|s| {
        s.setup = Arc::new(MatchSetup {
            seed: SeedSource::Plain(7),
            ..Default::default()
        })
    });
    for (i, variant) in variants.iter().enumerate() {
        assert_ne!(variant.hash(), state.hash(), "variant {i}");
    }
}

#[test]
fn clone_is_independent_of_original() {
    let original = base();
    let before = original.hash();
    let mut copy = original.clone();
    copy.figures[0].location = Some(Space::new(1, 0, 0));
    copy.flow.phases.push(PhaseFrame::default());
    copy.units[0].tokens.push(Token {
        kind: DefId(2),
        visibility: Visibility::Hidden,
    });
    assert_eq!(original.hash(), before);
    assert_eq!(original, base());
    assert_ne!(copy.hash(), before);
}

#[test]
fn commands_and_events_round_trip_through_json() {
    let command = Command::Action {
        figure: FigureId(1),
        action: DefId(5),
        args: Value::from(2),
    };
    let json = serde_json::to_string(&command).unwrap();
    assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), command);

    let event = Event::FigureMoved {
        figure: FigureId(0),
        to: Space::new(1, 0, 0),
        rotation: Rotation::from_steps(2),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), event);
}

#[test]
fn decisions_and_answers_round_trip_and_hash() {
    let decision = Decision {
        prompt: Sym::new("pick"),
        kind: DecisionKind::ChooseSpace(vec![Space::new(0, 0, 0)]),
    };
    let answer = Answer::Order(vec![1, 0]);
    let json = serde_json::to_string(&(&decision, &answer)).unwrap();
    let (d, a): (Decision, Answer) = serde_json::from_str(&json).unwrap();
    assert_eq!((d, a), (decision.clone(), answer.clone()));

    let mut state = base();
    let before = state.hash();
    state.pending = Some(PendingDecision {
        decision,
        chooser: PlayerId(0),
        command: Command::EndActivation,
        answers: vec![answer],
    });
    assert_ne!(state.hash(), before);
}

#[test]
fn visibility_tags() {
    assert_eq!(Command::EndActivation.visibility(), Visibility::Public);
    assert_eq!(
        Command::PlaceTokens(vec![]).visibility(),
        Visibility::Hidden
    );
    let placed = |visibility| Event::TokenPlaced {
        unit: UnitId(0),
        token: Token {
            kind: DefId(2),
            visibility,
        },
    };
    assert_eq!(placed(Visibility::Hidden).visibility(), Visibility::Hidden);
    assert_eq!(placed(Visibility::Public).visibility(), Visibility::Public);
    assert_eq!(
        Event::PhaseEntered(DefId(1)).visibility(),
        Visibility::Public
    );
}
