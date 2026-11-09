use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

use bevy::prelude::*;
use strata_kernel::{Space, Sym, Value};
use strata_model::{
    Answer, ArmyEntry, ArmyList, Command, Decision, DecisionKind, Event, FigureId, FigureState,
    FigureStatus, GameState, PendingDecision, PlayerId, Text, UnitId, UnitState,
};
use strata_session::{LaunchSpec, PlayerView, SimHandle, SimMessage, launch};

use super::decision::{Highlights, Widget, reorder};
use super::pick::{Pick, Refusal};
use super::*;
use crate::sim_link::{EventLog, SendCommand, SimLinkPlugin, SimStatus};

// ---- fixtures -------------------------------------------------------------------------------

pub(super) fn figure(unit: u32, owner: u8, at: Option<Space>) -> FigureState {
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

pub(super) fn space(q: i32) -> Space {
    Space::new(q, 0, 1)
}

/// Unit 0 (player 0) has figures 0 and 1; unit 1 (player 1) has figures 2 and 3, with 3 in
/// reserve.
pub(super) fn state() -> GameState {
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

fn decision(kind: DecisionKind) -> Decision {
    Decision {
        prompt: Sym::new("pick"),
        kind,
    }
}

pub(super) fn figures(ids: &[u32]) -> BTreeSet<FigureId> {
    ids.iter().copied().map(FigureId).collect()
}

// ---- decision to widget ---------------------------------------------------------------------

#[test]
fn each_decision_kind_maps_to_its_widget() {
    let syms = |xs: &[&str]| xs.iter().map(|x| Sym::new(x)).collect::<Vec<_>>();
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
fn a_decision_with_no_valid_answer_says_so_instead_of_an_empty_widget() {
    let s = state();
    let kinds = [
        DecisionKind::ChooseFigure(vec![]),
        DecisionKind::ChooseSpace(vec![]),
        DecisionKind::ChooseOption(vec![]),
        DecisionKind::Order(vec![]),
        DecisionKind::Number { min: 3, max: 1 },
    ];
    for kind in kinds {
        let widget = Widget::of(&decision(kind));
        assert_eq!(widget, Widget::NoChoices);
        assert_eq!(widget.highlights(), Highlights::default());
        assert_eq!(
            widget.click(&Pick::at(&s, space(0))),
            Err(Refusal::NotAChoice)
        );
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

// ---- shared input ---------------------------------------------------------------------------

#[test]
fn a_pending_decision_takes_board_clicks_and_one_answer_goes_out() {
    let mut s = state();
    s.pending = Some(PendingDecision {
        decision: decision(DecisionKind::ChooseFigure(vec![FigureId(1)])),
        chooser: PlayerId(0),
        player: PlayerId(0),
        command: Command::EndActivation,
        answers: Vec::new(),
    });
    let mut input = DecisionInput::of(&PlayerView::of(&s, PlayerId(0)));
    assert_eq!(input.highlights().figures, figures(&[1]));
    assert_eq!(input.click(&Pick::at(&s, space(0))), None);
    assert_eq!(input.why, Some(Refusal::NotAChoice));
    let answer = Some(Answer::Figure(FigureId(1)));
    assert_eq!(input.click(&Pick::at(&s, space(1))), answer);
    assert_eq!(input.why, None);
    assert!(input.sent);
    assert_eq!(input.click(&Pick::at(&s, space(1))), None);
    assert_eq!(input.highlights(), Highlights::default());
}

#[test]
fn another_seats_decision_is_answered_here_too() {
    // Every seat is a human on this client's channels until T-056, so player 1's decision is
    // answered from player 0's screen rather than blocking the match.
    let mut s = state();
    s.pending = Some(PendingDecision {
        decision: decision(DecisionKind::ChooseFigure(vec![FigureId(3)])),
        chooser: PlayerId(1),
        player: PlayerId(1),
        command: Command::EndActivation,
        answers: Vec::new(),
    });
    let mut input = DecisionInput::of(&PlayerView::of(&s, PlayerId(0)));
    assert_eq!(input.widget, Some(Widget::Figures(figures(&[3]))));
    let answer = input.click(&Pick::figure(FigureId(3)));
    assert_eq!(answer, Some(Answer::Figure(FigureId(3))));
}

#[test]
fn with_no_decision_a_click_does_nothing_and_says_nothing() {
    let s = state();
    let mut input = DecisionInput::of(&PlayerView::of(&s, PlayerId(0)));
    assert_eq!(input.widget, None);
    assert_eq!(input.highlights(), Highlights::default());
    assert_eq!(input.click(&Pick::at(&s, space(0))), None);
    assert_eq!(input.why, None);
}

/// A headless app with the decision input and the resources the sim link would provide.
fn input_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<LatestView>()
        .init_resource::<SimStatus>()
        .add_message::<SendAnswer>()
        .add_plugins(PlayerInputPlugin);
    app
}

fn pending_view() -> PlayerView {
    let mut s = state();
    s.pending = Some(PendingDecision {
        decision: decision(DecisionKind::ChooseFigure(vec![FigureId(0)])),
        chooser: PlayerId(0),
        player: PlayerId(0),
        command: Command::EndActivation,
        answers: Vec::new(),
    });
    PlayerView::of(&s, PlayerId(0))
}

#[test]
fn the_input_resets_once_the_match_stops() {
    for stopped in [SimStatus::Finished, SimStatus::Failed("boom".into())] {
        let mut app = input_app();
        app.world_mut().resource_mut::<LatestView>().0 = Some(Box::new(pending_view()));
        app.update();
        app.world_mut().resource_mut::<DecisionInput>().sent = true;
        app.update();
        assert!(app.world().resource::<DecisionInput>().sent);
        *app.world_mut().resource_mut::<SimStatus>() = stopped;
        app.update();
        assert_eq!(
            *app.world().resource::<DecisionInput>(),
            DecisionInput::default()
        );
        assert_eq!(app.world().resource::<BoardHighlights>().0, []);
    }
}

/// Whether [`DecisionInput`] and [`Highlighted`] changed in the last frame, as a system sees it.
#[derive(Resource, Default)]
struct Changes(bool, bool);

fn record_changes(input: Res<DecisionInput>, shown: Res<Highlighted>, mut seen: ResMut<Changes>) {
    *seen = Changes(input.is_changed(), shown.is_changed());
}

#[test]
fn idle_frames_do_not_mark_the_input_or_highlights_changed() {
    let mut app = input_app();
    app.init_resource::<Changes>()
        .add_systems(PostUpdate, record_changes);
    app.world_mut().resource_mut::<LatestView>().0 = Some(Box::new(pending_view()));
    app.update();
    let seen = app.world().resource::<Changes>();
    assert!(seen.0 && seen.1, "the first view changes both");
    app.update();
    let seen = app.world().resource::<Changes>();
    assert!(!seen.0 && !seen.1, "an idle frame changes neither");
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
/// `rally` ability asks player 0 to pick the attacker or the target of an attack. The match
/// allows enough commands to deploy every figure first.
fn decide() -> SimHandle {
    let root = env!("CARGO_MANIFEST_DIR");
    launch(LaunchSpec {
        roots: vec![PathBuf::from(root).join("../../fixtures/packs/decide")],
        scenario: "duel".into(),
        ruleset: Some("rs".into()),
        seed: 3,
        armies: vec![army("test_soldier"), army("test_archer")],
        commands: 5,
    })
}

/// Where the end-to-end test deploys figure `figure`: one per meadow of the strip.
fn deployed(figure: u32) -> Space {
    Space::new(figure as i32, 0, 1)
}

/// The toy driver's `move_to`, which puts a figure from reserve on the board.
fn move_to(figure: u32) -> Command {
    let at = deployed(figure);
    let payload = [
        (Sym::new("figure"), Value::Int(i64::from(figure))),
        (Sym::new("q"), Value::Int(i64::from(at.q))),
        (Sym::new("r"), Value::Int(i64::from(at.r))),
        (Sym::new("z"), Value::Int(i64::from(at.z))),
    ];
    Command::Custom {
        id: Sym::new("move_to"),
        payload: Value::Map(payload.into()),
    }
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
    loop {
        match handle.recv_timeout(WAIT) {
            Ok(SimMessage::View(view)) => return *view,
            Ok(SimMessage::Board(_)) => {}
            other => panic!("expected a view, got {other:?}"),
        }
    }
}

/// The next view, skipping the events that come before it.
fn view_after_events(handle: &SimHandle) -> PlayerView {
    loop {
        match handle.recv_timeout(WAIT) {
            Ok(SimMessage::View(view)) => return *view,
            Ok(SimMessage::Events(_) | SimMessage::Board(_)) => {}
            other => panic!("expected a view, got {other:?}"),
        }
    }
}

#[test]
fn a_lua_choose_with_figures_highlights_exactly_the_legal_figures() {
    let handle = decide();
    let _ = next_view(&handle);
    for figure in 0..4 {
        handle.send_command(move_to(figure));
        let view = view_after_events(&handle);
        let placed = &view.state.figures[figure as usize];
        assert_eq!(placed.status, FigureStatus::OnBoard);
        assert_eq!(placed.location, Some(deployed(figure)));
    }
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

    let mut input = DecisionInput::of(&view);
    assert_eq!(input.highlights().figures, legal);
    assert_eq!(input.highlights().spaces, BTreeSet::new());
    let outlined = input.highlights().board_spaces(&view.state);
    assert_eq!(outlined, [deployed(0), deployed(2)]);

    let state = &view.state;
    assert_eq!(input.click(&Pick::at(state, deployed(1))), None);
    assert_eq!(input.why, Some(Refusal::NotAChoice));
    let Some(answer) = input.click(&Pick::at(state, deployed(2))) else {
        panic!("a click on a legal figure's space answers the decision");
    };
    assert_eq!(answer, Answer::Figure(FigureId(2)));
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
        .add_plugins(PlayerInputPlugin);
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
    let input = app.world().resource::<DecisionInput>();
    assert!(matches!(input.widget, Some(Widget::Figures(_))));
}

// ---- end to end: a real match from deployment to an action ----------------------------------

/// Runs frames until `ready` holds for the app, failing after a while.
fn until(app: &mut App, what: &str, ready: impl Fn(&App) -> bool) {
    let mut frames = 0;
    while !ready(app) {
        app.update();
        frames += 1;
        assert!(frames < 2000, "{what} never happened");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn custom(id: &str, payload: Value) -> Command {
    Command::Custom {
        id: Sym::new(id),
        payload,
    }
}

/// The `command` fixture: player 0 deploys one soldier, places its token, activates it by a
/// click on the board and walks; player 1 has no army.
#[test]
fn a_real_match_is_played_from_the_client_through_tokens_activation_and_an_action() {
    let root = env!("CARGO_MANIFEST_DIR");
    let mut soldier = army("test_soldier");
    if let Some(list) = &mut soldier {
        list.units[0].copies = 1;
    }
    let handle = launch(LaunchSpec {
        roots: vec![PathBuf::from(root).join("../../fixtures/packs/command")],
        scenario: "duel".into(),
        ruleset: Some("rs".into()),
        seed: 5,
        armies: vec![soldier, None],
        commands: 6,
    });
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(SimLinkPlugin::new(handle))
        .add_plugins(PlayerInputPlugin);
    let index = |app: &App| {
        let view = app.world().resource::<LatestView>().0.as_deref();
        view.map(|v| v.state.command_index)
    };
    until(&mut app, "the first view", |app| index(app) == Some(0));
    let at = Value::List(vec![Value::List([0, 0, 1].map(Value::Int).to_vec())]);
    let place = [(Sym::new("unit"), Value::Int(0)), (Sym::new("at"), at)];
    let deploy = custom("place_unit", Value::Map(place.into()));
    app.world_mut().write_message(SendCommand(deploy));
    until(&mut app, "deployment", |app| index(app) == Some(1));
    let start = custom("start_round", Value::Nil);
    app.world_mut().write_message(SendCommand(start));

    // The token placement screen: a click on the soldier puts the only token on its unit.
    let input = |app: &App| app.world().resource::<CommandInput>().clone();
    until(&mut app, "token placement", |app| {
        input(app).tokens.is_some()
    });
    let soldier = Space::new(0, 0, 1);
    assert_eq!(
        app.world().resource::<Highlighted>().0.figures,
        figures(&[0])
    );
    app.world_mut().write_message(SpaceClicked(soldier));
    app.update();
    let placement = input(&app).tokens.and_then(|t| t.command());
    let placement = placement.expect("the click placed the whole hand");
    app.world_mut().write_message(SendCommand(placement));

    // The turn: a click on the soldier activates its unit, a second selects it to act.
    until(&mut app, "the turn", |app| {
        input(app).builder.step() == builder::Step::Unit
    });
    assert_eq!(
        app.world().resource::<Highlighted>().0.figures,
        figures(&[0])
    );
    app.world_mut().write_message(SpaceClicked(soldier));
    until(&mut app, "the activation", |app| {
        input(app).builder.step() == builder::Step::Figure
    });
    app.world_mut().write_message(SpaceClicked(soldier));
    app.update();
    let mut built = input(&app);
    assert_eq!(built.builder.step(), builder::Step::Action(FigureId(0)));
    let walk = built.builder.actions()[0];
    let names = app
        .world()
        .resource::<LatestView>()
        .0
        .as_ref()
        .unwrap()
        .names
        .clone();
    assert_eq!(names.action(walk), Some("walk"));
    let action = built
        .builder
        .choose_action(walk)
        .expect("walk takes no target");
    assert!(matches!(
        action,
        Command::Action {
            figure: FigureId(0),
            ..
        }
    ));
    app.world_mut().write_message(SendCommand(action));
    let walked = |e: &Event| matches!(e, Event::Custom { id, .. } if id.as_str() == "walked");
    until(&mut app, "the walk", |app| {
        app.world().resource::<EventLog>().0.iter().any(walked)
    });
}
