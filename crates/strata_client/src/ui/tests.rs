use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use strata_kernel::{DefId, Sym, Value};
use strata_model::{
    ArmyEntry, ArmyList, Event, GameState, PlayerId, PlayerState, Text, Token, UnitId, UnitState,
    Visibility,
};
use strata_session::{LaunchSpec, Names, PlayerView, SimMessage, launch};

use super::cards::{Tone, card_models};
use super::log::{describe, lines};
use super::turn::{TokenModel, TurnModel};

fn army() -> Option<ArmyList> {
    Some(ArmyList {
        name: Text("test".into()),
        format: Sym::new("none"),
        packs: BTreeMap::new(),
        units: vec![ArmyEntry {
            unit: Sym::new("test_soldier"),
            copies: 1,
        }],
    })
}

/// The first view of a match on the `cards` fixture, whose ruleset adds 1 to every defense.
fn launched_view() -> PlayerView {
    let root = env!("CARGO_MANIFEST_DIR");
    let handle = launch(LaunchSpec {
        roots: vec![PathBuf::from(root).join("../../fixtures/packs/cards")],
        scenario: "duel".into(),
        ruleset: Some("rs".into()),
        seed: 1,
        armies: vec![army(), army()],
        commands: 0,
    });
    match handle.recv_timeout(Duration::from_secs(60)) {
        Ok(SimMessage::View(view)) => *view,
        other => panic!("expected a view, got {other:?}"),
    }
}

#[test]
fn a_stat_modified_by_an_aura_shows_its_modified_value() {
    let models = card_models(&launched_view());
    let card = &models[0];
    let stat = |label: &str| card.stats.iter().find(|s| s.label == label).unwrap();
    assert_eq!((stat("DEF").value, stat("DEF").tone), (3, Tone::Buffed));
    assert_eq!((stat("ATK").value, stat("ATK").tone), (3, Tone::Plain));
    assert_eq!(card.abilities[0].text, "Adds 1 to defense.");
}

#[test]
fn a_card_counts_figures_alive_and_wounds() {
    let mut view = launched_view();
    assert!(card_models(&view)[0].mine);
    let figures = &mut view.cards[0].figures;
    figures[0].alive = false;
    figures[1].wounds = 2;
    let card = &card_models(&view)[0];
    assert_eq!((card.alive, card.total, card.wounds), (1, 2, 2));
}

#[test]
fn a_card_knows_whether_the_viewer_owns_the_unit() {
    let models = card_models(&launched_view());
    assert_eq!(models.len(), 2);
    assert_eq!(models.iter().filter(|c| c.mine).count(), 1);
}

fn unit(owner: u8, kind: u32) -> UnitState {
    UnitState {
        owner: PlayerId(owner),
        tokens: vec![Token {
            kind: DefId(kind),
            visibility: Visibility::Hidden,
        }],
        ..UnitState::default()
    }
}

#[test]
fn the_turn_tracker_shows_own_tokens_and_hides_opponents() {
    let player = |hand: Vec<DefId>| PlayerState {
        hand,
        ..PlayerState::default()
    };
    let mut state = GameState {
        units: vec![unit(0, 7), unit(1, 8)],
        players: vec![player(vec![DefId(1), DefId(2)]), player(Vec::new())],
        ..GameState::default()
    };
    state.flow.round = 2;
    state.flow.turn = 3;
    state.flow.active = PlayerId(1);
    let turn = TurnModel::of(&PlayerView::of(&state, PlayerId(0)));
    assert_eq!((turn.round, turn.turn, turn.active), (2, 3, 2));
    assert!(!turn.my_turn);
    assert_eq!(turn.hand, 2);
    assert_eq!(turn.rows[0].tokens, [TokenModel::Known(7)]);
    assert!(turn.rows[0].mine);
    assert_eq!(turn.rows[1].tokens, [TokenModel::Hidden]);
    assert!(!turn.rows[1].mine);
}

#[test]
fn the_log_keeps_player_facing_events_and_drops_bookkeeping() {
    let events = [
        Event::RoundStarted(1),
        Event::VarSet {
            key: Default::default(),
            value: Value::Int(1),
        },
        Event::WoundsApplied {
            figure: strata_model::FigureId(4),
            wound: DefId(0),
            amount: 2,
        },
        Event::ActivationEnded,
        Event::TokenRevealed {
            unit: UnitId(3),
            index: 0,
        },
    ];
    assert_eq!(
        lines(&events, &Names::default()),
        [
            "Round 1 begins",
            "Figure 4 takes 2 #0",
            "A token is revealed on unit 3",
        ]
    );
    assert_eq!(describe(&Event::ActivationEnded, &Names::default()), None);
}

/// The ID `names` gives to the die, marker or phase called `name`.
fn id_of(name: &str, lookup: impl Fn(DefId) -> Option<String>) -> DefId {
    (0..64)
        .map(DefId)
        .find(|id| lookup(*id).as_deref() == Some(name))
        .unwrap()
}

#[test]
fn the_log_words_dice_and_markers_with_their_names() {
    let names = launched_view().names;
    let die = id_of("Combat", |id| names.die(id).map(str::to_owned));
    let wound = id_of("Wound", |id| names.marker(id).map(str::to_owned));
    let events = [
        Event::DiceRolled {
            die,
            results: vec![1, 2],
        },
        Event::WoundsApplied {
            figure: strata_model::FigureId(4),
            wound,
            amount: 2,
        },
        Event::MarkerChanged {
            figure: strata_model::FigureId(1),
            marker: wound,
            delta: -1,
            source: None,
            expiry: None,
        },
    ];
    assert_eq!(
        lines(&events, &names),
        [
            "Combat rolled: [1, 2]",
            "Figure 4 takes 2 Wound",
            "Figure 1 -1 Wound",
        ]
    );
}

#[test]
fn drawing_the_prompt_with_no_input_leaves_the_decision_input_unchanged() {
    use bevy_egui::egui;
    use strata_model::{Command, Decision, DecisionKind, PendingDecision};

    use crate::input::DecisionInput;

    let syms = |xs: &[&str]| xs.iter().map(|x| Sym::new(x)).collect::<Vec<_>>();
    let kinds = [
        DecisionKind::YesNo,
        DecisionKind::Number { min: 1, max: 4 },
        DecisionKind::Order(syms(&["a", "b", "c"])),
        DecisionKind::ChooseOption(syms(&["left", "right"])),
    ];
    for kind in kinds {
        let pending = PendingDecision {
            decision: Decision {
                prompt: Sym::new("pick"),
                kind,
            },
            chooser: PlayerId(1),
            player: PlayerId(1),
            command: Command::EndActivation,
            answers: Vec::new(),
        };
        let state = GameState {
            pending: Some(pending),
            ..GameState::default()
        };
        let view = PlayerView::of(&state, PlayerId(0));
        let mut input = DecisionInput::of(&view);
        let before = input.clone();
        let ctx = egui::Context::default();
        let mut answer = None;
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                answer = super::prompt::decision(ui, &view, &mut input);
            });
            output.textures_delta.clear();
        }
        assert_eq!(answer, None);
        assert_eq!(input, before);
    }
}
