//! The simulation thread, its channels and the random controller.

use std::time::Duration;

use strata_kernel::Sym;
use strata_model::{
    Answer, Command, Decision, DecisionKind, PlayerId, RulesetDef, ScenarioDef, SeedSource,
};

use crate::pack_test::PackTests;
use crate::pack_test::host::find;
use crate::tests::{Rig, attack, pack_tests, seat, setup};
use crate::*;

const WAIT: Duration = Duration::from_secs(60);

/// A random controller that, where the toy ruleset offers no flow and so no legal commands,
/// chooses among a fixed set of attacks instead.
struct Toy(RandomController);

impl Controller for Toy {
    fn choose_command(&mut self, view: &PlayerView, legal: &[Command]) -> Command {
        let attacks = [attack(0, 2), attack(1, 3), attack(0, 3)];
        let legal = if legal.is_empty() {
            &attacks[..]
        } else {
            legal
        };
        self.0.choose_command(view, legal)
    }

    fn answer(&mut self, view: &PlayerView, decision: &Decision) -> Answer {
        self.0.answer(view, decision)
    }
}

/// Plays `commands` commands between two random controllers on the sim thread and returns every
/// message up to and including `Finished`.
fn random_match(seed: u64, commands: u32) -> Vec<SimMessage> {
    let handle = SimHandle::spawn(move |link| {
        let tests = pack_tests();
        let rig = Rig::new(&tests, setup(&tests)).unwrap();
        let seats: Vec<Box<dyn Controller>> = (0..2)
            .map(|seat| Box::new(Toy(RandomController::new(seed, seat))) as Box<dyn Controller>)
            .collect();
        let mut session = Session::new(rig.engine(), seats);
        link.run(&mut session, PlayerId(0), commands);
    });
    let mut messages = Vec::new();
    loop {
        let message = handle.recv_timeout(WAIT).expect("the match stalled");
        let done = matches!(message, SimMessage::Finished(_) | SimMessage::Error(_));
        messages.push(message);
        if done {
            break;
        }
    }
    handle.shutdown().unwrap();
    messages
}

fn final_hash(messages: &[SimMessage]) -> u64 {
    let last = messages.iter().rev().find_map(|m| match m {
        SimMessage::View(view) => Some(view.state.hash()),
        _ => None,
    });
    last.unwrap()
}

#[test]
fn two_random_controllers_play_a_match_through_the_channels() {
    let messages = random_match(3, 12);
    assert!(matches!(messages.last(), Some(SimMessage::Finished(None))));
    let events = messages
        .iter()
        .filter(|m| matches!(m, SimMessage::Events(_)));
    assert_eq!(events.count(), 12);
    let last_view = messages.iter().rev().find_map(|m| match m {
        SimMessage::View(view) => Some(view),
        _ => None,
    });
    assert_eq!(last_view.unwrap().state.command_index, 12);
}

#[test]
fn the_same_seed_gives_the_same_hash_sequence() {
    let hashes = |seed| {
        let messages = random_match(seed, 12);
        let views = messages.iter().filter_map(|m| match m {
            SimMessage::View(view) => Some(view.state.hash()),
            _ => None,
        });
        views.collect::<Vec<_>>()
    };
    assert_eq!(hashes(9), hashes(9));
    assert_eq!(final_hash(&random_match(9, 12)), *hashes(9).last().unwrap());
}

#[test]
fn shutting_down_joins_a_thread_waiting_on_a_human() {
    let handle = SimHandle::spawn(|link| {
        let tests = pack_tests();
        let rig = Rig::new(&tests, setup(&tests)).unwrap();
        let seats: Vec<Box<dyn Controller>> = vec![Box::new(link.human()), Box::new(link.human())];
        let mut session = Session::new(rig.engine(), seats);
        link.run(&mut session, PlayerId(0), 100);
    });
    // The first message is the starting view; after that the thread blocks on the human.
    assert!(matches!(handle.recv_timeout(WAIT), Ok(SimMessage::View(_))));
    assert!(handle.try_recv().is_none());
    assert_eq!(handle.shutdown(), Ok(()));
}

#[test]
fn a_human_seat_plays_the_commands_the_client_sends() {
    let handle = SimHandle::spawn(|link| {
        let tests = pack_tests();
        let rig = Rig::new(&tests, setup(&tests)).unwrap();
        let seats: Vec<Box<dyn Controller>> = vec![Box::new(link.human()), Box::new(link.human())];
        let mut session = Session::new(rig.engine(), seats);
        link.run(&mut session, PlayerId(0), 1);
    });
    assert!(matches!(handle.recv_timeout(WAIT), Ok(SimMessage::View(_))));
    handle.send_command(attack(0, 2));
    assert!(matches!(handle.recv_timeout(WAIT), Ok(SimMessage::Events(e)) if !e.is_empty()));
    assert!(
        matches!(handle.recv_timeout(WAIT), Ok(SimMessage::View(v)) if v.state.command_index == 1)
    );
    assert!(matches!(
        handle.recv_timeout(WAIT),
        Ok(SimMessage::Finished(None))
    ));
    assert_eq!(handle.shutdown(), Ok(()));
}

fn decision(kind: DecisionKind) -> Decision {
    Decision {
        prompt: Sym::new("pick"),
        kind,
    }
}

#[test]
fn random_answers_stay_inside_the_decision() {
    let tests = pack_tests();
    let state = crate::build_match(&tests.registry, setup(&tests))
        .unwrap()
        .state;
    let view = PlayerView::of(&state, PlayerId(0));
    let mut random = RandomController::new(5, 0);
    for _ in 0..200 {
        let options = decision(DecisionKind::ChooseOption(vec![
            Sym::new("a"),
            Sym::new("b"),
        ]));
        assert!(matches!(
            random.answer(&view, &options),
            Answer::Option(0 | 1)
        ));
        let number = decision(DecisionKind::Number { min: -2, max: 2 });
        assert!(matches!(
            random.answer(&view, &number),
            Answer::Number(-2..=2)
        ));
        let order = decision(DecisionKind::Order(vec![
            Sym::new("a"),
            Sym::new("b"),
            Sym::new("c"),
        ]));
        let Answer::Order(mut picked) = random.answer(&view, &order) else {
            panic!("expected an ordering");
        };
        picked.sort();
        assert_eq!(picked, [0, 1, 2]);
    }
}

#[test]
fn random_choices_cover_every_legal_command() {
    let tests = pack_tests();
    let state = crate::build_match(&tests.registry, setup(&tests))
        .unwrap()
        .state;
    let view = PlayerView::of(&state, PlayerId(0));
    let legal = [attack(0, 2), attack(1, 3), attack(0, 3)];
    let mut random = RandomController::new(1, 0);
    let mut seen = [false; 3];
    for _ in 0..100 {
        let pick = random.choose_command(&view, &legal);
        seen[legal.iter().position(|c| *c == pick).unwrap()] = true;
    }
    assert_eq!(seen, [true; 3]);
}

/// The `victory` fixture pack, with a setup on `scenario` that fields a test_soldier pair a
/// side.
fn victory_setup(scenario: &str) -> (PackTests, strata_model::MatchSetup) {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [std::path::PathBuf::from(root).join("../../fixtures/packs/victory")];
    let loaded = PackTests::load(&roots).unwrap();
    let modules = strata_modules::native_modules().into_iter();
    let tests = modules.fold(loaded, |t, m| t.with_native(m).unwrap());
    let registry = &tests.registry;
    let (ruleset, _) = find::<RulesetDef>(registry, "victory", "rs").unwrap();
    let (scenario, _) = find::<ScenarioDef>(registry, "victory", scenario).unwrap();
    let setup = strata_model::MatchSetup {
        packs: pack_identities(registry),
        ruleset,
        format: None,
        scenario,
        seed: SeedSource::Plain(4),
        players: vec![seat(0, "a", "test_soldier"), seat(1, "b", "test_soldier")],
    };
    (tests, setup)
}

/// A random controller that starts a round when nothing is legal, since a rig's engine has no
/// flow to offer commands.
struct Rounds(RandomController);

impl Controller for Rounds {
    fn choose_command(&mut self, view: &PlayerView, legal: &[Command]) -> Command {
        if legal.is_empty() {
            return Command::Custom {
                id: Sym::new("start_round"),
                payload: strata_kernel::Value::Nil,
            };
        }
        self.0.choose_command(view, legal)
    }

    fn answer(&mut self, view: &PlayerView, decision: &Decision) -> Answer {
        self.0.answer(view, decision)
    }
}

fn random_seats(seed: u64) -> Vec<Box<dyn Controller>> {
    let seat = |n| Box::new(Rounds(RandomController::new(seed, n))) as Box<dyn Controller>;
    vec![seat(0), seat(1)]
}

#[test]
fn a_session_stops_asking_once_the_match_is_over() {
    let (tests, setup) = victory_setup("first_command");
    let rig = Rig::new(&tests, setup).unwrap();
    let mut session = Session::new(rig.engine(), random_seats(2));
    session.run(1000).unwrap();
    let result = session.result().expect("the first command ends the match");
    assert_eq!((result.condition.as_str(), result.index), ("script", 0));
    assert_eq!(result.winners, [PlayerId(0)]);
    let played = session.state().command_index;
    assert_eq!(played, 1);
    assert_eq!(session.step(), Ok(Vec::new()));
    assert_eq!(session.state().command_index, played);
}

#[test]
fn the_sim_finishes_with_the_result_once_the_match_is_over() {
    let handle = SimHandle::spawn(|link| {
        let (tests, setup) = victory_setup("first_command");
        let rig = Rig::new(&tests, setup).unwrap();
        let mut session = Session::new(rig.engine(), random_seats(5));
        link.run(&mut session, PlayerId(0), 1000);
    });
    let mut messages = Vec::new();
    loop {
        let message = handle.recv_timeout(WAIT).expect("the match stalled");
        let done = matches!(message, SimMessage::Finished(_) | SimMessage::Error(_));
        messages.push(message);
        if done {
            break;
        }
    }
    handle.shutdown().unwrap();
    let Some(SimMessage::Finished(Some(result))) = messages.last() else {
        panic!("expected a finished match, got {:?}", messages.last());
    };
    assert_eq!(result.condition.as_str(), "script");
    assert_eq!(result.winners, [PlayerId(0)]);
    assert!(!messages.iter().any(|m| matches!(m, SimMessage::Error(_))));
}

#[test]
fn a_malformed_victory_condition_fails_setup() {
    let (tests, mut setup) = victory_setup("short_draw");
    let registry = &tests.registry;
    for (name, wanted) in [
        ("bad_zone", "the map has no zone `nowhere`"),
        ("bad_tiebreak", "unknown tiebreak `initiative_roll`"),
        (
            "bad_kind",
            "core:victory has no condition `king_of_the_hill`",
        ),
    ] {
        setup.scenario = find::<ScenarioDef>(registry, "victory", name).unwrap().0;
        let Err(error) = Rig::new(&tests, setup.clone()) else {
            panic!("`{name}` set up");
        };
        assert!(error.0.contains(wanted), "{name}: {error}");
    }
}
