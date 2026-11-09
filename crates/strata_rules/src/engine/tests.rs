use std::collections::BTreeMap;
use std::sync::Arc;

use strata_kernel::{CommandRng, DefId, ModuleId, Sym, Value, WindowId};
use strata_model::{
    Answer, Command, Decision, DecisionKind, Descriptor, Event, FigureId, FigureState, GameState,
    Logic, MatchSetup, PlayerId, PlayerState, ReactionDef, ReplacementDef, SeedSource, TriggerDef,
    TriggerLimit, VarKey,
};

use super::*;
use crate::registrar::{CommandHandler, Registrar};

type Body = fn(&mut dyn Ctx, PlayerId) -> Result<(), Halt>;

/// A command handler whose behavior is a plain function.
struct Toy(&'static str, Body);

impl CommandHandler for Toy {
    fn kind(&self) -> &str {
        self.0
    }

    fn resolve(&self, ctx: &mut dyn Ctx, player: PlayerId, _: &Command) -> Result<(), Halt> {
        (self.1)(ctx, player)
    }
}

/// Replacements and triggers are numbered by the integer in their logic.
struct ToyHooks;

fn number(logic: &Logic) -> i64 {
    match logic {
        Logic::Native(Descriptor(Value::Int(i))) => *i,
        _ => panic!("toy hooks only know numbered logic"),
    }
}

fn wounded(event: &Event) -> i64 {
    match event {
        Event::WoundsApplied { figure, .. } => i64::from(figure.0),
        _ => 0,
    }
}

impl HookRunner for ToyHooks {
    fn replaces(&self, _: &ReplacementDef, _: &Event, _: &GameState) -> Result<bool, Halt> {
        Ok(true)
    }

    /// Turns `VarSet(v)` into `VarSet(v * 10 + n + 1)`, so the order of replacements shows.
    fn replace(
        &self,
        def: &ReplacementDef,
        e: &Event,
        _: &mut dyn Ctx,
    ) -> Result<Vec<Event>, Halt> {
        let Event::VarSet {
            key,
            value: Value::Int(v),
        } = e
        else {
            return Ok(vec![e.clone()]);
        };
        let value = Value::Int(v * 10 + number(&def.replace) + 1);
        Ok(vec![Event::VarSet {
            key: key.clone(),
            value,
        }])
    }

    fn fires(&self, _: &TriggerDef, _: &Event, _: &GameState) -> Result<bool, Halt> {
        Ok(true)
    }

    /// Emits `Custom(fired, n * 100 + figure)`.
    /// The reaction's `when` names its player.
    fn reactor(&self, def: &ReactionDef, _: &GameState) -> Result<Option<PlayerId>, Halt> {
        let who = def.when.as_ref().map(number);
        Ok(who.map(|p| PlayerId(p as u8)))
    }

    /// Emits `Custom(reacted, n)`; reaction 9 tries to reopen its own window.
    fn react(&self, def: &ReactionDef, ctx: &mut dyn Ctx) -> Result<(), Halt> {
        if number(&def.effect) == 9 {
            ctx.window(&WindowId::new("w"))?;
        }
        ctx.emit(Event::Custom {
            id: Sym::new("reacted"),
            payload: Value::Int(number(&def.effect)),
        })
    }

    fn fire(&self, def: &TriggerDef, e: &Event, ctx: &mut dyn Ctx) -> Result<(), Halt> {
        let payload = Value::Int(number(&def.effect) * 100 + wounded(e));
        ctx.emit(Event::Custom {
            id: Sym::new("fired"),
            payload,
        })
    }
}

fn logic(n: i64) -> Logic {
    Logic::Native(Descriptor(Value::Int(n)))
}

fn replacement(n: i64) -> ReplacementDef {
    ReplacementDef {
        event: Sym::new("var_set"),
        filter: None,
        replace: logic(n),
    }
}

fn trigger(n: i64, limit: Option<TriggerLimit>) -> TriggerDef {
    TriggerDef {
        event: Sym::new("wounds_applied"),
        filter: None,
        limit,
        effect: logic(n),
    }
}

fn runtime(
    commands: Vec<Toy>,
    replacements: Vec<ReplacementDef>,
    triggers: Vec<TriggerDef>,
) -> RulesRuntime {
    let mut reg = Registrar::default();
    commands.into_iter().for_each(|c| reg.command(c));
    replacements.into_iter().for_each(|r| reg.replacement(r));
    triggers.into_iter().for_each(|t| reg.trigger(t));
    RulesRuntime {
        slots: BTreeMap::new(),
        addons: Vec::new(),
        configs: Vec::new(),
        reg,
    }
}

/// Two players; figure 0 belongs to player 1 and figure 1 to player 0, who is active.
fn state() -> GameState {
    let figure = |controller| FigureState {
        controller: PlayerId(controller),
        ..Default::default()
    };
    GameState {
        setup: Arc::new(MatchSetup {
            seed: SeedSource::Plain(7),
            ..Default::default()
        }),
        players: vec![PlayerState::default(); 2],
        figures: vec![figure(1), figure(0)],
        ..Default::default()
    }
}

fn var() -> VarKey {
    VarKey {
        module: ModuleId::new("t"),
        name: Sym::new("v"),
    }
}

fn set_var(n: i64) -> Event {
    Event::VarSet {
        key: var(),
        value: Value::Int(n),
    }
}

fn wound(figure: u32) -> Event {
    Event::WoundsApplied {
        figure: FigureId(figure),
        wound: DefId(1),
        amount: 1,
    }
}

fn cmd(id: &str) -> Command {
    Command::Custom {
        id: Sym::new(id),
        payload: Value::Nil,
    }
}

fn yes_no() -> Decision {
    Decision {
        prompt: Sym::new("sure"),
        kind: DecisionKind::YesNo,
    }
}

fn fired(events: &[Event]) -> Vec<i64> {
    let custom = |e: &Event| match e {
        Event::Custom {
            payload: Value::Int(n),
            ..
        } => Some(*n),
        _ => None,
    };
    events.iter().filter_map(custom).collect()
}

fn committed(outcome: Outcome) -> Vec<Event> {
    match outcome {
        Outcome::Committed(events) => events,
        other => panic!("expected a commit, got {other:?}"),
    }
}

/// Rolls, asks two questions, then records what it learned.
fn two_decisions(ctx: &mut dyn Ctx, _: PlayerId) -> Result<(), Halt> {
    let roll = ctx.rng().roll(6);
    ctx.emit(Event::DiceRolled {
        die: DefId(0),
        results: vec![roll],
    })?;
    let Answer::YesNo(yes) = ctx.choose(PlayerId(0), yes_no())? else {
        unreachable!()
    };
    let number = Decision {
        prompt: Sym::new("how many"),
        kind: DecisionKind::Number { min: 1, max: 6 },
    };
    let Answer::Number(n) = ctx.choose(PlayerId(1), number)? else {
        unreachable!()
    };
    ctx.emit(set_var(i64::from(n) + i64::from(yes)))
}

fn play_two_decisions() -> (Vec<Event>, Vec<CommandRecord>, GameState) {
    let rt = runtime(vec![Toy("two", two_decisions)], vec![], vec![]);
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    let first = engine.submit(PlayerId(0), cmd("two"));
    assert_eq!(
        first,
        Outcome::NeedsDecision {
            decision: yes_no(),
            chooser: PlayerId(0)
        }
    );
    assert!(matches!(
        engine.answer(Answer::YesNo(true)),
        Outcome::NeedsDecision {
            chooser: PlayerId(1),
            ..
        }
    ));
    let events = committed(engine.answer(Answer::Number(3)));
    (events, engine.log().to_vec(), engine.state().clone())
}

#[test]
fn a_two_decision_command_commits_after_both_answers() {
    let (events, log, state) = play_two_decisions();
    assert_eq!(events.last(), Some(&set_var(4)));
    assert_eq!(state.pending, None);
    assert_eq!(state.command_index, 1);
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].answers, vec![Answer::YesNo(true), Answer::Number(3)]);
    assert_eq!(log[0].state_hash, state.hash());
}

#[test]
fn rerunning_with_the_same_tape_gives_identical_events_and_dice() {
    let (events, log, state) = play_two_decisions();
    let expected = CommandRng::for_command(7, 0).roll(6);
    assert_eq!(
        events[0],
        Event::DiceRolled {
            die: DefId(0),
            results: vec![expected]
        }
    );

    let (events2, log2, state2) = play_two_decisions();
    assert_eq!((events, log, state.hash()), (events2, log2, state2.hash()));
}

#[test]
fn a_pending_decision_is_part_of_the_state() {
    let rt = runtime(vec![Toy("two", two_decisions)], vec![], vec![]);
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    let before = engine.state().hash();
    engine.submit(PlayerId(0), cmd("two"));
    assert_ne!(engine.state().hash(), before);
    assert!(engine.state().pending.is_some());
    assert!(matches!(
        engine.submit(PlayerId(0), cmd("two")),
        Outcome::Rejected(_)
    ));
}

#[test]
fn an_answer_that_does_not_fit_is_rejected_and_the_question_stays() {
    let rt = runtime(vec![Toy("two", two_decisions)], vec![], vec![]);
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    engine.submit(PlayerId(0), cmd("two"));
    engine.answer(Answer::YesNo(false));
    let pending = engine.state().pending.clone();
    assert!(matches!(
        engine.answer(Answer::Number(9)),
        Outcome::Rejected(_)
    ));
    assert!(matches!(
        engine.answer(Answer::YesNo(true)),
        Outcome::Rejected(_)
    ));
    assert_eq!(engine.state().pending, pending);
}

fn illegal(ctx: &mut dyn Ctx, _: PlayerId) -> Result<(), Halt> {
    ctx.emit(set_var(1))?;
    ctx.emit(wound(1))?;
    Err(Halt::reject("not allowed"))
}

#[test]
fn a_rejected_command_leaves_state_untouched() {
    let rt = runtime(vec![Toy("bad", illegal)], vec![], vec![]);
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    let before = engine.state().clone();
    let outcome = engine.submit(PlayerId(0), cmd("bad"));
    assert_eq!(
        outcome,
        Outcome::Rejected(RuleViolation("not allowed".into()))
    );
    assert_eq!(engine.state(), &before);
    assert!(engine.log().is_empty());
    assert!(matches!(
        engine.submit(PlayerId(0), cmd("nobody")),
        Outcome::Rejected(_)
    ));
    assert_eq!(engine.state(), &before);
}

fn set_zero(ctx: &mut dyn Ctx, _: PlayerId) -> Result<(), Halt> {
    ctx.emit(set_var(0))
}

#[test]
fn a_replacement_applies_at_most_once_per_event() {
    // Each replacement yields another `var_set`; without the once-per-event rule it would loop.
    let rt = runtime(vec![Toy("set", set_zero)], vec![replacement(0)], vec![]);
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    let events = committed(engine.submit(PlayerId(0), cmd("set")));
    assert_eq!(events, vec![set_var(1)]);
    assert_eq!(engine.state().vars.get(&var()), Some(&Value::Int(1)));
}

#[test]
fn the_affected_controller_orders_several_replacements() {
    let rt = runtime(
        vec![Toy("set", set_zero)],
        vec![replacement(0), replacement(1)],
        vec![],
    );
    for (order, expected) in [(vec![0, 1], 12), (vec![1, 0], 21)] {
        let mut engine = Engine::new(&rt, &ToyHooks, state());
        let Outcome::NeedsDecision { decision, chooser } = engine.submit(PlayerId(0), cmd("set"))
        else {
            panic!("expected a decision");
        };
        assert_eq!(chooser, PlayerId(0));
        assert!(matches!(decision.kind, DecisionKind::Order(ref items) if items.len() == 2));
        let events = committed(engine.answer(Answer::Order(order)));
        assert_eq!(events, vec![set_var(expected)]);
    }
}

fn wound_both(ctx: &mut dyn Ctx, _: PlayerId) -> Result<(), Halt> {
    ctx.emit(wound(0))?;
    ctx.emit(wound(1))
}

#[test]
fn triggers_resolve_active_player_first() {
    let rt = runtime(
        vec![Toy("hurt", wound_both)],
        vec![],
        vec![trigger(0, None)],
    );
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    let events = committed(engine.submit(PlayerId(0), cmd("hurt")));
    // Figure 0 was wounded first but belongs to the inactive player 1.
    assert_eq!(fired(&events), vec![1, 0]);
}

fn wound_one(ctx: &mut dyn Ctx, _: PlayerId) -> Result<(), Halt> {
    ctx.emit(wound(1))
}

#[test]
fn a_player_orders_their_simultaneous_triggers() {
    let triggers = vec![trigger(0, None), trigger(1, None)];
    let rt = runtime(vec![Toy("hurt", wound_one)], vec![], triggers);
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    let Outcome::NeedsDecision { chooser, .. } = engine.submit(PlayerId(0), cmd("hurt")) else {
        panic!("expected a decision");
    };
    assert_eq!(chooser, PlayerId(0));
    let events = committed(engine.answer(Answer::Order(vec![1, 0])));
    assert_eq!(fired(&events), vec![101, 1]);
}

#[test]
fn a_trigger_limit_stops_a_second_firing() {
    let limited = trigger(0, Some(TriggerLimit::OncePerRound));
    let rt = runtime(vec![Toy("hurt", wound_both)], vec![], vec![limited]);
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    let events = committed(engine.submit(PlayerId(0), cmd("hurt")));
    assert_eq!(fired(&events).len(), 1);
}

fn nag(ctx: &mut dyn Ctx, _: PlayerId) -> Result<(), Halt> {
    loop {
        ctx.choose(PlayerId(0), yes_no())?;
    }
}

#[test]
fn a_command_asking_too_many_decisions_is_rejected() {
    let rt = runtime(vec![Toy("nag", nag)], vec![], vec![]);
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    let before = engine.state().clone();
    let mut outcome = engine.submit(PlayerId(0), cmd("nag"));
    let mut answered = 0;
    while matches!(outcome, Outcome::NeedsDecision { .. }) {
        answered += 1;
        outcome = engine.answer(Answer::YesNo(true));
    }
    assert!(matches!(outcome, Outcome::Rejected(_)));
    assert_eq!(answered, MAX_DECISIONS);
    assert_eq!(engine.state(), &before);
}

#[test]
fn legal_commands_start_empty() {
    let rt = runtime(vec![], vec![], vec![]);
    assert!(
        Engine::new(&rt, &ToyHooks, state())
            .legal_commands(PlayerId(0))
            .is_empty()
    );
}

fn reaction(player: i64, n: i64) -> ReactionDef {
    ReactionDef {
        window: WindowId::new("w"),
        when: Some(logic(player)),
        effect: logic(n),
    }
}

fn open_w(ctx: &mut dyn Ctx, _: PlayerId) -> Result<(), Halt> {
    ctx.window(&WindowId::new("w"))
}

fn open_missing(ctx: &mut dyn Ctx, _: PlayerId) -> Result<(), Halt> {
    ctx.window(&WindowId::new("nope"))
}

fn window_runtime(reactions: Vec<ReactionDef>) -> RulesRuntime {
    let mut rt = runtime(
        vec![Toy("open", open_w), Toy("missing", open_missing)],
        vec![],
        vec![],
    );
    rt.reg.window(WindowId::new("w"));
    reactions.into_iter().for_each(|r| rt.reg.reaction(r));
    rt
}

fn offered(outcome: &Outcome) -> (PlayerId, usize) {
    match outcome {
        Outcome::NeedsDecision {
            decision:
                Decision {
                    kind: DecisionKind::ChooseOption(options),
                    ..
                },
            chooser,
        } => (*chooser, options.len()),
        other => panic!("expected an option decision, got {other:?}"),
    }
}

fn reacted(events: &[Event]) -> Vec<i64> {
    let is_reaction =
        |e: &&Event| matches!(e, Event::Custom { id, .. } if id.as_ref() == "reacted");
    fired(
        &events
            .iter()
            .filter(is_reaction)
            .cloned()
            .collect::<Vec<_>>(),
    )
}

#[test]
fn a_window_with_no_eligible_reactions_costs_no_decision() {
    let rt = window_runtime(vec![]);
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    assert_eq!(committed(engine.submit(PlayerId(0), cmd("open"))), vec![]);
}

#[test]
fn reactions_from_two_players_resolve_active_player_first() {
    let rt = window_runtime(vec![reaction(1, 2), reaction(0, 1)]);
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    let first = engine.submit(PlayerId(0), cmd("open"));
    assert_eq!(offered(&first), (PlayerId(0), 2));
    let second = engine.answer(Answer::Option(1));
    assert_eq!(offered(&second), (PlayerId(1), 2));
    let events = committed(engine.answer(Answer::Option(1)));
    assert_eq!(reacted(&events), vec![1, 2]);
}

#[test]
fn a_declined_reaction_is_not_offered_again() {
    let rt = window_runtime(vec![reaction(0, 1), reaction(1, 2)]);
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    engine.submit(PlayerId(0), cmd("open"));
    let second = engine.answer(Answer::Option(0));
    assert_eq!(offered(&second), (PlayerId(1), 2));
    let events = committed(engine.answer(Answer::Option(1)));
    assert_eq!(reacted(&events), vec![2]);
}

#[test]
fn a_used_reaction_is_not_offered_twice() {
    let rt = window_runtime(vec![reaction(0, 1)]);
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    engine.submit(PlayerId(0), cmd("open"));
    let events = committed(engine.answer(Answer::Option(1)));
    assert_eq!(reacted(&events), vec![1]);
}

#[test]
fn a_window_cannot_be_opened_inside_itself() {
    let rt = window_runtime(vec![reaction(0, 9)]);
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    engine.submit(PlayerId(0), cmd("open"));
    let Outcome::Rejected(RuleViolation(why)) = engine.answer(Answer::Option(1)) else {
        panic!("expected a rejection");
    };
    assert!(why.contains("already open"), "{why}");
}

#[test]
fn an_undeclared_window_is_rejected() {
    let rt = window_runtime(vec![]);
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    assert!(matches!(
        engine.submit(PlayerId(0), cmd("missing")),
        Outcome::Rejected(_)
    ));
}

#[test]
fn window_decisions_replay_identically() {
    let play = || {
        let rt = window_runtime(vec![reaction(0, 1), reaction(1, 2)]);
        let mut engine = Engine::new(&rt, &ToyHooks, state());
        engine.submit(PlayerId(0), cmd("open"));
        engine.answer(Answer::Option(1));
        engine.answer(Answer::Option(1));
        engine
            .log()
            .iter()
            .map(|r| r.state_hash)
            .collect::<Vec<_>>()
    };
    assert_eq!(play(), play());
}

/// Offers every player a hand of one decoy for unit 0.
struct OneToken;

impl crate::flow::TokenRules for OneToken {
    fn choice(&self, _: &GameState, _: PlayerId) -> Option<strata_model::TokenChoice> {
        Some(strata_model::TokenChoice {
            hand: vec![DefId(0)],
            units: vec![strata_model::UnitId(0)],
        })
    }
}

#[test]
fn the_token_choice_comes_from_the_tokens_service_unless_a_decision_is_pending() {
    let bare = runtime(vec![], vec![], vec![]);
    assert_eq!(
        Engine::new(&bare, &ToyHooks, state()).token_choice(PlayerId(0)),
        None
    );
    let mut rt = runtime(vec![], vec![], vec![]);
    rt.reg
        .service::<dyn crate::flow::TokenRules>(Box::new(OneToken));
    let engine = Engine::new(&rt, &ToyHooks, state());
    let offered = engine
        .token_choice(PlayerId(0))
        .expect("the active player has a token");
    assert_eq!(offered.hand, [DefId(0)]);
    let mut paused = state();
    paused.flow.active = PlayerId(1);
    paused.pending = Some(PendingDecision {
        decision: Decision {
            prompt: Sym::new("p"),
            kind: DecisionKind::YesNo,
        },
        chooser: PlayerId(1),
        player: PlayerId(1),
        command: Command::EndActivation,
        answers: Vec::new(),
    });
    let engine = Engine::new(&rt, &ToyHooks, paused);
    assert_eq!(engine.token_choice(PlayerId(1)), None);
}

#[test]
fn only_the_active_player_is_offered_a_token_choice() {
    let mut rt = runtime(vec![], vec![], vec![]);
    rt.reg
        .service::<dyn crate::flow::TokenRules>(Box::new(OneToken));
    let mut second = state();
    second.flow.active = PlayerId(1);
    let engine = Engine::new(&rt, &ToyHooks, second);
    assert_eq!(
        engine.token_choice(PlayerId(0)),
        None,
        "player 0 waits while player 1 places"
    );
    assert!(engine.token_choice(PlayerId(1)).is_some());
}

/// Asks a native decision with an empty figure list, which nothing could answer.
fn empty_figures(ctx: &mut dyn Ctx, _: PlayerId) -> Result<(), Halt> {
    let none = Decision {
        prompt: Sym::new("pick"),
        kind: DecisionKind::ChooseFigure(Vec::new()),
    };
    ctx.choose(PlayerId(0), none)?;
    ctx.emit(set_var(1))
}

/// Asks a native number decision whose range is empty.
fn inverted_number(ctx: &mut dyn Ctx, _: PlayerId) -> Result<(), Halt> {
    let none = Decision {
        prompt: Sym::new("how many"),
        kind: DecisionKind::Number { min: 3, max: 1 },
    };
    ctx.choose(PlayerId(0), none)?;
    ctx.emit(set_var(1))
}

#[test]
fn a_native_decision_with_no_valid_answer_is_rejected_not_left_pending() {
    let rt = runtime(
        vec![
            Toy("empty", empty_figures),
            Toy("inverted", inverted_number),
        ],
        vec![],
        vec![],
    );
    let mut engine = Engine::new(&rt, &ToyHooks, state());
    let before = engine.state().clone();
    assert_eq!(
        engine.submit(PlayerId(0), cmd("empty")),
        Outcome::Rejected(RuleViolation("the decision offers no figures".into()))
    );
    assert_eq!(
        engine.submit(PlayerId(0), cmd("inverted")),
        Outcome::Rejected(RuleViolation("number min 3 is greater than max 1".into()))
    );
    assert_eq!(engine.state(), &before);
    assert_eq!(engine.state().pending, None);
}
