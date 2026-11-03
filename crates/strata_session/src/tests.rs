//! Matches on the toy pack: setup, scripted play, views, saves and replays.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::OnceLock;

use strata_kernel::{DefId, ModuleId, SlotId, Space, Sym, Value};
use strata_model::{
    Answer, ArmyEntry, ArmyList, Command, Decision, DecisionKind, Event, GameState, MatchSetup,
    ModuleRef, PlayerId, PlayerSetup, ReactionDef, ReplacementDef, RulesetDef, ScenarioDef,
    SeedSource, SourceLoc, Text, Token, TriggerDef, UndoPolicy, UnitId, Visibility,
};
use strata_modules::native_modules;
use strata_rules::{
    CommandHandler, ConfigSchema, Ctx, Engine, Halt, HookRunner, ModuleCatalog, ModuleError,
    Registrar, RuleModule, RulesetSource, resolve,
};

use crate::pack_test::PackTests;
use crate::pack_test::host::find;
use crate::*;

pub(crate) fn pack_tests() -> PackTests {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/toy")];
    let loaded = PackTests::load(&roots).unwrap();
    let modules = native_modules().into_iter();
    modules.fold(loaded, |t, m| t.with_native(m).unwrap())
}

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

fn seat(id: u8, slot: &str, unit: &str) -> PlayerSetup {
    PlayerSetup {
        id: PlayerId(id),
        slot: Sym::new(slot),
        team: id,
        army: army(unit),
    }
}

pub(crate) fn setup(tests: &PackTests) -> MatchSetup {
    let registry = &tests.registry;
    let (ruleset, _) = find::<RulesetDef>(registry, "toy", "toy_rust").unwrap();
    let (scenario, _) = find::<ScenarioDef>(registry, "toy", "duel").unwrap();
    MatchSetup {
        packs: pack_identities(registry),
        ruleset,
        format: None,
        scenario,
        seed: SeedSource::Plain(7),
        players: vec![seat(0, "a", "test_soldier"), seat(1, "b", "test_archer")],
    }
}

pub(crate) use crate::pack_test::MatchRig as Rig;

/// Plays its commands in a loop and answers every question with the first option.
struct Script {
    commands: Vec<Command>,
    next: usize,
}

impl Controller for Script {
    fn choose_command(&mut self, _: &PlayerView, _: &[Command]) -> Command {
        let command = self.commands[self.next % self.commands.len()].clone();
        self.next += 1;
        command
    }

    fn answer(&mut self, _: &PlayerView, _: &Decision) -> Answer {
        Answer::Option(0)
    }
}

pub(crate) fn attack(attacker: i64, target: i64) -> Command {
    let payload = [
        (Sym::new("attacker"), Value::Int(attacker)),
        (Sym::new("target"), Value::Int(target)),
    ];
    Command::Custom {
        id: Sym::new("attack"),
        payload: Value::Map(payload.into()),
    }
}

/// Player 0 volleys; player 1 is never asked, because player 0 stays the active player.
fn controllers() -> Vec<Box<dyn Controller>> {
    let volley = vec![attack(0, 2), attack(1, 3), attack(0, 3)];
    let idle = vec![attack(2, 0)];
    let seats = [volley, idle];
    seats
        .into_iter()
        .map(|commands| Box::new(Script { commands, next: 0 }) as Box<dyn Controller>)
        .collect()
}

/// Plays `commands` commands and returns the replay and the final state hash.
pub(crate) fn play(tests: &PackTests, commands: u32) -> (Replay, u64) {
    let rig = Rig::new(tests, setup(tests)).unwrap();
    let mut session = Session::new(rig.engine(), controllers());
    session.run(commands).unwrap();
    (session.replay(), session.state().hash())
}

fn replay_on_fresh_engine(tests: &PackTests, replay: &Replay) -> Result<(), ReplayError> {
    let rig = Rig::new(tests, replay.setup.clone()).unwrap();
    replay.play(&mut rig.engine())
}

#[test]
fn setup_deploys_armies_into_reserve_and_places_relics() {
    let tests = pack_tests();
    let state = build_match(&tests.registry, setup(&tests)).unwrap().state;
    assert_eq!(state.players.len(), 2);
    assert_eq!(state.units.len(), 4);
    assert_eq!(state.figures.len(), 4);
    assert!(state.figures.iter().all(|f| f.location.is_none()));
    let owners: Vec<u8> = state.units.iter().map(|u| u.owner.0).collect();
    assert_eq!(owners, [0, 0, 1, 1]);
    assert_eq!(state.units[1].copy, 1);
    // The relic spot is a meadow one level thick.
    assert_eq!(state.relics.len(), 1);
    assert_eq!(state.relics[0].location, Some(Space::new(5, 5, 1)));
    assert_eq!(state.setup.players.len(), 2);
}

#[test]
fn setup_rejects_bad_seats() {
    let tests = pack_tests();
    let good = setup(&tests);
    let mut lone = good.clone();
    lone.players.truncate(1);
    let mut twice = good.clone();
    twice.players[1].slot = Sym::new("a");
    let mut unknown = good.clone();
    unknown.players[1].slot = Sym::new("zzz");
    let mut shuffled = good.clone();
    shuffled.players.reverse();
    for bad in [lone, twice, unknown, shuffled] {
        assert!(build_match(&tests.registry, bad).is_err());
    }
    assert!(build_match(&tests.registry, good).is_ok());
}

#[test]
fn replaying_a_saved_match_reproduces_every_state_hash() {
    let tests = pack_tests();
    let (replay, last) = play(&tests, 25);
    assert_eq!(replay.commands.len(), 25);
    assert_eq!(replay.commands.last().unwrap().state_hash, last);
    // One snapshot, after the twentieth command.
    let at: Vec<u32> = replay.snapshots.iter().map(|s| s.command_index).collect();
    assert_eq!(at, [20]);

    let bytes = replay.save().unwrap();
    let loaded = Replay::load(&bytes).unwrap();
    assert_eq!(loaded, replay);
    loaded.verify(&tests.registry).unwrap();
    replay_on_fresh_engine(&tests, &loaded).unwrap();
}

#[test]
fn the_same_setup_and_scripts_give_identical_hash_sequences() {
    let tests = pack_tests();
    let (first, _) = play(&tests, 25);
    let (second, _) = play(&tests, 25);
    assert_eq!(first, second);
    let hashes = first.commands.iter().map(|c| c.state_hash);
    assert_eq!(hashes.collect::<BTreeSet<_>>().len(), 25);
}

#[test]
fn a_replay_that_no_longer_matches_is_refused() {
    let tests = pack_tests();
    let (replay, _) = play(&tests, 25);

    let mut wrong_hash = replay.clone();
    wrong_hash.commands[3].state_hash ^= 1;
    let got = replay_on_fresh_engine(&tests, &wrong_hash).unwrap_err();
    assert!(matches!(got, ReplayError::HashMismatch { index: 3, .. }));

    let mut wrong_snapshot = replay.clone();
    wrong_snapshot.snapshots[0].state.units[0].copy ^= 1;
    let got = replay_on_fresh_engine(&tests, &wrong_snapshot).unwrap_err();
    assert_eq!(got, ReplayError::SnapshotMismatch { index: 20 });

    let mut wrong_pack = replay.clone();
    wrong_pack.setup.packs[0].hash ^= 1;
    assert_eq!(wrong_pack.verify(&tests.registry), Err(ReplayError::Packs));

    let mut wrong_engine = replay.clone();
    wrong_engine.engine_version = "0.0.0".into();
    let got = wrong_engine.verify(&tests.registry).unwrap_err();
    assert!(matches!(got, ReplayError::Version { .. }));

    let mut wrong_seed = replay.clone();
    wrong_seed.setup.seed = SeedSource::Plain(8);
    let rig = Rig::new(&tests, setup(&tests)).unwrap();
    let got = wrong_seed.play(&mut rig.engine());
    assert_eq!(got, Err(ReplayError::WrongStart));

    let junk = Replay::load(&[1, 2, 3]);
    assert!(matches!(junk, Err(ReplayError::Format(_))));
}

fn token(kind: u32, visibility: Visibility) -> Token {
    Token {
        kind: DefId(kind),
        visibility,
    }
}

#[test]
fn views_hide_an_opponents_tokens_and_hands_but_not_the_count() {
    let tests = pack_tests();
    let mut state = build_match(&tests.registry, setup(&tests)).unwrap().state;
    state.units[0].tokens = vec![token(4, Visibility::Hidden), token(5, Visibility::Public)];
    state.units[2].tokens = vec![token(6, Visibility::Hidden)];
    state.players[1].hand = vec![DefId(8), DefId(9)];

    let mine = PlayerView::of(&state, PlayerId(0)).state;
    assert_eq!(mine.units[0].tokens[0].kind, DefId(4));
    assert_eq!(mine.units[2].tokens.len(), 1);
    assert_eq!(mine.units[2].tokens[0].kind, REDACTED);
    assert_eq!(mine.players[1].hand, [REDACTED, REDACTED]);

    let theirs = PlayerView::of(&state, PlayerId(1)).state;
    assert_eq!(theirs.units[0].tokens[0].kind, REDACTED);
    assert_eq!(theirs.units[0].tokens[1].kind, DefId(5));
    assert_eq!(theirs.units[2].tokens[0].kind, DefId(6));
    assert_eq!(theirs.players[1].hand, [DefId(8), DefId(9)]);
}

#[test]
fn events_are_filtered_by_visibility_per_viewer() {
    let tests = pack_tests();
    let state = build_match(&tests.registry, setup(&tests)).unwrap().state;
    let placed = |visibility| Event::TokenPlaced {
        unit: UnitId(0),
        token: token(4, visibility),
    };
    let events = [
        placed(Visibility::Hidden),
        placed(Visibility::Public),
        Event::ActivationEnded,
    ];
    let owner = visible_events(&state, PlayerId(0), &events);
    assert_eq!(owner, events);
    let other = visible_events(&state, PlayerId(1), &events);
    let kind = |e: &Event| match e {
        Event::TokenPlaced { token, .. } => Some(token.kind),
        _ => None,
    };
    let kinds: Vec<_> = other.iter().map(kind).collect();
    assert_eq!(kinds, [Some(REDACTED), Some(DefId(4)), None]);
    assert_eq!(other[2], Event::ActivationEnded);
}

/// A module whose `ask` command puts a question to player 1.
struct Ask;

struct AskHandler;

impl CommandHandler for AskHandler {
    fn kind(&self) -> &str {
        "ask"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, _: &Command) -> Result<(), Halt> {
        let question = Decision {
            prompt: Sym::new("proceed"),
            kind: DecisionKind::ChooseOption(vec![Sym::new("yes"), Sym::new("no")]),
        };
        let Answer::Option(picked) = ctx.choose(PlayerId(1), question)? else {
            return Err(Halt::reject("expected an option"));
        };
        ctx.emit(Event::Custom {
            id: Sym::new("answered"),
            payload: Value::Int(i64::from(picked)),
        })
    }
}

impl RuleModule for Ask {
    fn id(&self) -> ModuleId {
        ModuleId::new("t:ask")
    }

    fn slot(&self) -> Option<SlotId> {
        None
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        static EMPTY: OnceLock<ConfigSchema> = OnceLock::new();
        EMPTY.get_or_init(ConfigSchema::default)
    }

    fn register(&self, r: &mut Registrar, _: &Value) -> Result<(), ModuleError> {
        r.command(AskHandler);
        Ok(())
    }
}

struct AskRuleset(RulesetDef);

impl RulesetSource for AskRuleset {
    fn ruleset(&self, _: &str) -> Option<&RulesetDef> {
        Some(&self.0)
    }
}

/// Answers with the second option, and checks that it was shown its own view.
struct Chooser;

impl Controller for Chooser {
    fn choose_command(&mut self, _: &PlayerView, _: &[Command]) -> Command {
        panic!("only player 0 submits commands");
    }

    fn answer(&mut self, view: &PlayerView, _: &Decision) -> Answer {
        assert_eq!(view.viewer, PlayerId(1));
        Answer::Option(1)
    }
}

#[test]
fn a_pending_decision_goes_to_its_chooser_and_the_match_replays() {
    let mut catalog = ModuleCatalog::default();
    catalog.register(Box::new(Ask)).unwrap();
    let ruleset = RulesetDef {
        addons: vec![ModuleRef {
            id: ModuleId::new("t:ask"),
            config: BTreeMap::new(),
        }],
        id: DefId(0),
        name: Text::default(),
        extends: None,
        stats: Vec::new(),
        dice: Vec::new(),
        slots: BTreeMap::new(),
        abilities: true,
        undo: UndoPolicy::default(),
        tags: BTreeSet::new(),
        meta: BTreeMap::new(),
        source: SourceLoc::default(),
    };
    let rt = resolve("ask", &AskRuleset(ruleset), &catalog).unwrap();
    let start = GameState {
        players: vec![Default::default(), Default::default()],
        ..GameState::default()
    };
    let ask = Command::Custom {
        id: Sym::new("ask"),
        payload: Value::Nil,
    };
    let script = Script {
        commands: vec![ask],
        next: 0,
    };
    let seats: Vec<Box<dyn Controller>> = vec![Box::new(script), Box::new(Chooser)];
    let mut session = Session::new(Engine::new(&rt, &NoHooks, start.clone()), seats);

    assert!(session.step().unwrap().is_empty());
    assert!(session.state().pending.is_some());
    let events = session.step().unwrap();
    assert_eq!(events.len(), 1);
    assert!(session.state().pending.is_none());

    let replay = session.replay();
    assert_eq!(replay.commands[0].answers, [Answer::Option(1)]);
    replay.play(&mut Engine::new(&rt, &NoHooks, start)).unwrap();
}

/// Hooks for a ruleset with no triggers, replacements or reactions.
struct NoHooks;

impl HookRunner for NoHooks {
    fn replaces(&self, _: &ReplacementDef, _: &Event, _: &GameState) -> Result<bool, Halt> {
        Ok(false)
    }

    fn replace(&self, _: &ReplacementDef, e: &Event, _: &mut dyn Ctx) -> Result<Vec<Event>, Halt> {
        Ok(vec![e.clone()])
    }

    fn fires(&self, _: &TriggerDef, _: &Event, _: &GameState) -> Result<bool, Halt> {
        Ok(false)
    }

    fn fire(&self, _: &TriggerDef, _: &Event, _: &mut dyn Ctx) -> Result<(), Halt> {
        Ok(())
    }

    fn reactor(&self, _: &ReactionDef, _: &GameState) -> Result<Option<PlayerId>, Halt> {
        Ok(None)
    }

    fn react(&self, _: &ReactionDef, _: &mut dyn Ctx) -> Result<(), Halt> {
        Ok(())
    }
}
