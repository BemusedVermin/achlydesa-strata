use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

use bevy::prelude::*;
use strata_kernel::{DefId, Space, Sym, Value};
use strata_model::{
    Answer, ArmyEntry, ArmyList, Command, Decision, DecisionKind, Event, FigureId, FigureState,
    FigureStatus, GameState, PendingDecision, PlayerId, Text, UnitId, UnitState,
};
use strata_session::{LaunchSpec, PlayerView, SimHandle, SimMessage, launch};

use super::builder::{CommandBuilder, Step, Target};
use super::decision::{Highlights, Widget, reorder};
use super::pick::{Pick, Refusal};
use super::*;
use crate::sim_link::SimLinkPlugin;

// ---- fixtures -------------------------------------------------------------------------------

fn figure(unit: u32, owner: u8, at: Option<Space>) -> FigureState {
    FigureState {
        unit: UnitId(unit),
        controller: PlayerId(owner),
        status: if at.is_some() {
            FigureStatus::OnBoard
        } else {
            FigureStatus::Reserve
        },
        location: at,
        ..FigureState::default()
    }
}

fn space(q: i32) -> Space {
    Space::new(q, 0, 1)
}

/// Unit 0 (player 0) has figures 0 and 1; unit 1 (player 1) has figures 2 and 3, with 3 in
/// reserve.
fn state() -> GameState {
    let unit = |owner: u8, figures: [u32; 2]| UnitState {
        owner: PlayerId(owner),
        figures: figures.map(FigureId).to_vec(),
        ..UnitState::default()
    };
    GameState {
        units: vec![unit(0, [0, 1]), unit(1, [2, 3])],
        figures: vec![
            figure(0, 0, Some(space(0))),
            figure(0, 0, Some(space(1))),
            figure(1, 1, Some(space(3))),
            figure(1, 1, None),
        ],
        ..GameState::default()
    }
}

fn act(figure: u32, action: u32, args: Value) -> Command {
    Command::Action {
        figure: FigureId(figure),
        action: DefId(action),
        args,
    }
}

fn coords(q: i64, r: i64, z: i64) -> Value {
    Value::List(vec![Value::Int(q), Value::Int(r), Value::Int(z)])
}

/// Walk takes no target, shoot targets figure 2, step targets space (2, 0, 1).
fn activation_legal() -> Vec<Command> {
    vec![
        act(0, 0, Value::Nil),
        act(0, 1, Value::Int(2)),
        act(0, 2, coords(2, 0, 1)),
        act(1, 0, Value::Nil),
        Command::EndActivation,
    ]
}

fn decision(kind: DecisionKind) -> Decision {
    Decision {
        prompt: Sym::new("pick"),
        kind,
    }
}

fn figures(ids: &[u32]) -> BTreeSet<FigureId> {
    ids.iter().copied().map(FigureId).collect()
}

// ---- decision to widget ---------------------------------------------------------------------

#[test]
fn each_decision_kind_maps_to_its_widget() {
    let syms = |xs: &[&str]| xs.iter().map(|x| Sym::new(*x)).collect::<Vec<_>>();
    let cases = [
        (
            DecisionKind::ChooseFigure(vec![FigureId(2), FigureId(0)]),
            Widget::Figures(figures(&[0, 2])),
        ),
        (
            DecisionKind::ChooseSpace(vec![space(1)]),
            Widget::Spaces([space(1)].into()),
        ),
        (
            DecisionKind::ChooseOption(syms(&["left", "right"])),
            Widget::Options(vec!["left".into(), "right".into()]),
        ),
        (DecisionKind::YesNo, Widget::YesNo),
        (
            DecisionKind::Number { min: 1, max: 3 },
            Widget::Stepper { min: 1, max: 3 },
        ),
        (
            DecisionKind::Order(syms(&["a", "b"])),
            Widget::Order(vec!["a".into(), "b".into()]),
        ),
    ];
    for (kind, widget) in cases {
        assert_eq!(Widget::of(&decision(kind)), widget);
    }
}

#[test]
fn a_figure_choice_highlights_exactly_its_figures_and_explains_other_clicks() {
    let widget = Widget::of(&decision(DecisionKind::ChooseFigure(vec![
        FigureId(0),
        FigureId(2),
    ])));
    let s = state();
    let shown = widget.highlights();
    assert_eq!(shown.figures, figures(&[0, 2]));
    assert!(shown.spaces.is_empty());
    assert_eq!(shown.board_spaces(&s), [space(0), space(3)]);
    assert_eq!(
        widget.click(&Pick::at(&s, space(3))),
        Ok(Answer::Figure(FigureId(2)))
    );
    assert_eq!(
        widget.click(&Pick::at(&s, space(1))),
        Err(Refusal::NotAChoice)
    );
    assert_eq!(
        widget.click(&Pick::at(&s, space(2))),
        Err(Refusal::NoFigure)
    );
}

#[test]
fn a_space_choice_answers_only_its_spaces_and_dialogs_ignore_the_board() {
    let s = state();
    let widget = Widget::of(&decision(DecisionKind::ChooseSpace(vec![space(2)])));
    assert_eq!(widget.highlights().spaces, [space(2)].into());
    assert_eq!(
        widget.click(&Pick::at(&s, space(2))),
        Ok(Answer::Space(space(2)))
    );
    assert_eq!(
        widget.click(&Pick::at(&s, space(1))),
        Err(Refusal::NotAChoice)
    );
    let dialog = Widget::of(&decision(DecisionKind::YesNo));
    assert_eq!(dialog.highlights(), Highlights::default());
    assert_eq!(
        dialog.click(&Pick::at(&s, space(0))),
        Err(Refusal::UseDialog)
    );
}

#[test]
fn steppers_start_at_the_minimum_and_lists_in_given_order() {
    let stepper = Widget::Stepper { min: 2, max: 5 };
    assert_eq!(stepper.start(), (2, vec![]));
    let list = Widget::Order(vec!["a".into(), "b".into(), "c".into()]);
    let (_, mut order) = list.start();
    assert_eq!(order, [0, 1, 2]);
    reorder(&mut order, 2, 0);
    assert_eq!(order, [2, 0, 1]);
    reorder(&mut order, 0, 9);
    assert_eq!(order, [2, 0, 1]);
}

// ---- command building -----------------------------------------------------------------------

#[test]
fn nothing_legal_means_every_click_is_refused() {
    let mut builder = CommandBuilder::new(PlayerId(0), Vec::new());
    assert_eq!(builder.step(), Step::Idle);
    let refused = builder.click(&state(), &Pick::at(&state(), space(0)));
    assert_eq!(refused, Err(Refusal::NothingToDo));
}

#[test]
fn clicking_a_figure_activates_its_unit_when_that_is_legal() {
    let s = state();
    let mut builder = CommandBuilder::new(PlayerId(0), vec![Command::ActivateUnit(UnitId(0))]);
    assert_eq!(builder.step(), Step::Unit);
    assert_eq!(builder.highlights(&s).figures, figures(&[0, 1]));
    let click = |b: &mut CommandBuilder, q| b.click(&s, &Pick::at(&s, space(q)));
    assert_eq!(click(&mut builder, 3), Err(Refusal::NotYours));
    assert_eq!(click(&mut builder, 2), Err(Refusal::NoFigure));
    let activate = Some(Command::ActivateUnit(UnitId(0)));
    assert_eq!(click(&mut builder, 1), Ok(activate));
}

#[test]
fn a_unit_that_may_not_activate_says_so() {
    let s = state();
    let mut builder = CommandBuilder::new(PlayerId(1), vec![Command::ActivateUnit(UnitId(0))]);
    let pick = Pick::at(&s, space(3));
    assert_eq!(builder.click(&s, &pick), Err(Refusal::CannotActivate));
}

#[test]
fn figure_then_action_then_target_builds_a_legal_command() {
    let s = state();
    let mut builder = CommandBuilder::new(PlayerId(0), activation_legal());
    assert_eq!(builder.step(), Step::Figure);
    assert_eq!(builder.actors(), [FigureId(0), FigureId(1)]);
    assert_eq!(builder.highlights(&s).figures, figures(&[0, 1]));
    assert!(builder.can_end());

    let click = |b: &mut CommandBuilder, q| b.click(&s, &Pick::at(&s, space(q)));
    assert_eq!(click(&mut builder, 3), Err(Refusal::NotYours));
    assert_eq!(click(&mut builder, 0), Ok(None));
    assert_eq!(builder.step(), Step::Action(FigureId(0)));
    assert_eq!(builder.actions(), [DefId(0), DefId(1), DefId(2)]);

    assert_eq!(builder.choose_action(DefId(1)), None);
    assert_eq!(builder.step(), Step::Target(FigureId(0), DefId(1)));
    assert_eq!(builder.highlights(&s).figures, figures(&[2]));
    assert_eq!(click(&mut builder, 2), Err(Refusal::NotATarget));
    assert_eq!(click(&mut builder, 3), Ok(Some(act(0, 1, Value::Int(2)))));
    assert_eq!(builder.step(), Step::Figure);
}

#[test]
fn a_space_target_is_highlighted_and_taken_from_the_board() {
    let s = state();
    let mut builder = CommandBuilder::new(PlayerId(0), activation_legal());
    builder.click(&s, &Pick::figure(FigureId(0))).unwrap();
    builder.choose_action(DefId(2));
    let shown = builder.highlights(&s);
    assert_eq!(shown.spaces, [Space::new(2, 0, 1)].into());
    assert!(shown.figures.is_empty());
    let pick = Pick::at(&s, Space::new(2, 0, 1));
    assert_eq!(
        builder.click(&s, &pick),
        Ok(Some(act(0, 2, coords(2, 0, 1))))
    );
}

#[test]
fn an_untargeted_action_is_ready_at_once_and_back_undoes_a_step() {
    let s = state();
    let mut builder = CommandBuilder::new(PlayerId(0), activation_legal());
    assert_eq!(builder.choose_action(DefId(0)), None, "no figure yet");
    builder.click(&s, &Pick::figure(FigureId(0))).unwrap();
    builder.choose_action(DefId(1));
    builder.back();
    assert_eq!(builder.step(), Step::Action(FigureId(0)));
    builder.back();
    assert_eq!(builder.step(), Step::Figure);
    builder.click(&s, &Pick::figure(FigureId(1))).unwrap();
    assert_eq!(builder.actions(), [DefId(0)]);
    assert_eq!(
        builder.choose_action(DefId(1)),
        None,
        "not legal for figure 1"
    );
    assert_eq!(builder.choose_action(DefId(0)), Some(act(1, 0, Value::Nil)));
}

#[test]
fn a_figure_that_cannot_act_says_so_and_another_actor_can_be_reselected() {
    let s = state();
    let legal = vec![act(0, 1, Value::Int(2))];
    let mut builder = CommandBuilder::new(PlayerId(0), legal);
    assert_eq!(
        builder.click(&s, &Pick::figure(FigureId(1))),
        Err(Refusal::CannotAct)
    );
    builder.click(&s, &Pick::figure(FigureId(0))).unwrap();
    builder.choose_action(DefId(1));
    assert_eq!(builder.choose_option(0), Some(act(0, 1, Value::Int(2))));
}

#[test]
fn targets_read_figures_and_spaces_and_leave_the_rest_to_a_list() {
    let s = state();
    let map: BTreeMap<Sym, Value> = [("q", 1), ("r", 2), ("z", 3)]
        .map(|(k, v)| (Sym::new(k), Value::Int(v)))
        .into();
    assert_eq!(Target::of(&Value::Int(3), &s), Target::Figure(FigureId(3)));
    assert_eq!(Target::of(&Value::Int(7), &s), Target::Other(Value::Int(7)));
    assert_eq!(
        Target::of(&coords(1, 2, 3), &s),
        Target::Space(Space::new(1, 2, 3))
    );
    assert_eq!(
        Target::of(&Value::Map(map), &s),
        Target::Space(Space::new(1, 2, 3))
    );
    let below = coords(1, 2, -1);
    assert_eq!(Target::of(&below, &s), Target::Other(below.clone()));
}

// ---- shared input ---------------------------------------------------------------------------

fn view(state: GameState, legal: Vec<Command>) -> PlayerView {
    let mut view = PlayerView::of(&state, PlayerId(0));
    view.legal = legal;
    view
}

#[test]
fn a_pending_decision_takes_clicks_before_the_builder_and_one_reply_goes_out() {
    let mut s = state();
    s.pending = Some(PendingDecision {
        decision: decision(DecisionKind::ChooseFigure(vec![FigureId(1)])),
        chooser: PlayerId(0),
        player: PlayerId(0),
        command: Command::EndActivation,
        answers: Vec::new(),
    });
    let mut input = CommandInput::of(&view(s.clone(), Vec::new()));
    assert_eq!(input.highlights(&s).figures, figures(&[1]));
    assert_eq!(input.click(&s, &Pick::at(&s, space(0))), None);
    assert_eq!(input.why, Some(Refusal::NotAChoice));
    let answer = Some(Reply::Answer(Answer::Figure(FigureId(1))));
    assert_eq!(input.click(&s, &Pick::at(&s, space(1))), answer);
    assert_eq!(input.why, None);
    assert!(input.sent);
    assert_eq!(input.click(&s, &Pick::at(&s, space(1))), None);
    assert_eq!(input.highlights(&s), Highlights::default());
}

#[test]
fn another_players_decision_leaves_input_to_the_builder() {
    let mut s = state();
    s.pending = Some(PendingDecision {
        decision: decision(DecisionKind::YesNo),
        chooser: PlayerId(1),
        player: PlayerId(1),
        command: Command::EndActivation,
        answers: Vec::new(),
    });
    let input = CommandInput::of(&view(s, Vec::new()));
    assert_eq!(input.widget, None);
    assert_eq!(input.builder.step(), Step::Idle);
}

// ---- end to end: a Lua ability's ctx.choose -------------------------------------------------

const WAIT: Duration = Duration::from_secs(60);

fn army(unit: &str) -> Option<ArmyList> {
    Some(ArmyList {
        name: Text("test".into()),
        format: Sym::new("none"),
        packs: BTreeMap::new(),
        units: vec![ArmyEntry {
            unit: Sym::new(unit),
            copies: 2,
        }],
    })
}

/// The `decide` fixture: player 0 has figures 0 and 1, player 1 has figures 2 and 3. Its
/// `rally` ability asks player 0 to pick the attacker or the target of an attack.
fn decide() -> SimHandle {
    let root = env!("CARGO_MANIFEST_DIR");
    launch(LaunchSpec {
        roots: vec![PathBuf::from(root).join("../../fixtures/packs/decide")],
        scenario: "duel".into(),
        ruleset: Some("rs".into()),
        seed: 3,
        armies: vec![army("test_soldier"), army("test_archer")],
        commands: 1,
    })
}

fn attack(attacker: i64, target: i64) -> Command {
    let payload = [
        (Sym::new("attacker"), Value::Int(attacker)),
        (Sym::new("target"), Value::Int(target)),
    ];
    Command::Custom {
        id: Sym::new("attack"),
        payload: Value::Map(payload.into()),
    }
}

fn next_view(handle: &SimHandle) -> PlayerView {
    match handle.recv_timeout(WAIT) {
        Ok(SimMessage::View(view)) => *view,
        other => panic!("expected a view, got {other:?}"),
    }
}

#[test]
fn a_lua_choose_with_figures_highlights_exactly_the_legal_figures() {
    let handle = decide();
    let _ = next_view(&handle);
    handle.send_command(attack(0, 2));
    let view = next_view(&handle);
    let pending = view
        .state
        .pending
        .as_ref()
        .expect("the ability asks a question");
    let DecisionKind::ChooseFigure(legal) = &pending.decision.kind else {
        panic!("expected a figure choice, got {:?}", pending.decision.kind);
    };
    let legal: BTreeSet<FigureId> = legal.iter().copied().collect();
    assert_eq!(legal, figures(&[0, 2]));

    let mut input = CommandInput::of(&view);
    assert_eq!(input.highlights(&view.state).figures, legal);
    assert_eq!(input.highlights(&view.state).spaces, BTreeSet::new());

    let state = &view.state;
    assert_eq!(input.click(state, &Pick::figure(FigureId(1))), None);
    assert_eq!(input.why, Some(Refusal::NotAChoice));
    let Some(Reply::Answer(answer)) = input.click(state, &Pick::figure(FigureId(2))) else {
        panic!("a legal figure answers the decision");
    };
    handle.send_answer(answer);
    let rallied = |e: &Event| matches!(e, Event::Custom { id, payload } if id.as_str() == "rallied" && *payload == Value::Int(2));
    match handle.recv_timeout(WAIT) {
        Ok(SimMessage::Events(events)) => assert!(events.iter().any(rallied), "{events:?}"),
        other => panic!("expected events, got {other:?}"),
    }
}

#[test]
fn the_client_highlights_the_choices_of_a_decision_it_receives() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(SimLinkPlugin::new(decide()))
        .add_plugins(CommandInputPlugin);
    let mut frames = 0;
    let mut sent = false;
    while app.world().resource::<Highlighted>().0.figures.is_empty() {
        app.update();
        let ready = app.world().resource::<LatestView>().0.is_some();
        if ready && !sent {
            app.world_mut().write_message(SendCommand(attack(1, 3)));
            sent = true;
        }
        frames += 1;
        assert!(frames < 2000, "the decision never arrived");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        app.world().resource::<Highlighted>().0.figures,
        figures(&[1, 3])
    );
    let input = app.world().resource::<CommandInput>();
    assert!(matches!(input.widget, Some(Widget::Figures(_))));
}
