//! `core:command_tokens`, through the `tokens` fixture pack.

use std::path::PathBuf;
use std::sync::OnceLock;

use strata_kernel::{ModuleId, SlotId, Sym, Value};
use strata_model::{
    Command, Event, FigureId, GameState, PlayerId, PlayerState, UnitId, UnitState, Visibility,
};
use strata_modules::native_modules;
use strata_rules::{CommandHandler, ConfigSchema, Ctx, Halt, ModuleError, Registrar, RuleModule};
use strata_session::pack_test::{PackTests, TestReport, summary};
use strata_session::{PlayerView, REDACTED, visible_events};

/// Handles `pass`: the active player's activation starts and ends, which ends their turn.
struct PassHandler;

impl CommandHandler for PassHandler {
    fn kind(&self) -> &str {
        "pass"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, player: PlayerId, _: &Command) -> Result<(), Halt> {
        if ctx.state().flow.active != player {
            return Err(Halt::reject("it is not your turn"));
        }
        let unit = ctx.state().units.iter().position(|u| u.owner == player);
        let unit = unit.ok_or_else(|| Halt::reject("no unit"))?;
        ctx.emit(Event::ActivationStarted {
            unit: UnitId(unit as u32),
            step: Sym::new("go"),
            slots: Default::default(),
        })?;
        ctx.emit(Event::ActivationEnded)
    }
}

struct Pass;

impl RuleModule for Pass {
    fn id(&self) -> ModuleId {
        ModuleId::new("test:pass")
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
        r.command(PassHandler);
        Ok(())
    }
}

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/tokens")];
    let mut modules = native_modules();
    modules.push(Box::new(Pass));
    let loaded = PackTests::load(&roots).unwrap();
    let tests = modules.into_iter().try_fold(loaded, PackTests::with_native);
    tests.unwrap().run(None)
}

const ORDER: &str = "tokens are revealed in order and decide who acts";
const DESTROYED: &str = "a destroyed unit loses its token and its turn";
const FREE: &str = "an empty tokens slot skips placement";

fn report<'a>(reports: &'a [TestReport], what: &str) -> &'a TestReport {
    reports.iter().find(|r| r.name == what).unwrap()
}

fn revealed(report: &TestReport) -> Vec<u32> {
    let shown = report.events.iter().filter_map(|e| match e {
        Event::TokenRevealed { unit, .. } => Some(unit.0),
        _ => None,
    });
    shown.collect()
}

fn count(report: &TestReport, what: impl Fn(&Event) -> bool) -> usize {
    report.events.iter().filter(|e| what(e)).count()
}

fn is_named(event: &Event, name: &str) -> bool {
    matches!(event, Event::Custom { id, .. } if id.as_str() == name)
}

/// The match the `board()` of the pack starts from: three soldiers each, units 0-2 and 3-5.
fn start_state() -> GameState {
    let unit = |i: u32| UnitState {
        owner: PlayerId((i / 3) as u8),
        figures: vec![FigureId(i)],
        ..Default::default()
    };
    GameState {
        players: vec![PlayerState::default(); 2],
        units: (0..6).map(unit).collect(),
        figures: (0..6).map(|_| Default::default()).collect(),
        ..Default::default()
    }
}

/// The state after the first `n` events.
fn state_after(events: &[Event], n: usize) -> GameState {
    let mut state = start_state();
    for event in &events[..n] {
        state.apply(event).unwrap();
    }
    state
}

#[test]
fn every_pack_test_passes_except_the_three_that_expect_a_refusal() {
    let reports = run();
    let failed: Vec<&str> = reports
        .iter()
        .filter(|r| r.outcome.is_err())
        .map(|r| r.name.as_str())
        .collect();
    let expected = [
        "a player cannot place their tokens twice",
        "a token cannot go on an enemy unit",
        "tokens cannot be placed outside the placement phase",
    ];
    let mut sorted = failed.clone();
    sorted.sort();
    assert_eq!(sorted, expected, "{}", summary(&reports));
    let message = |name| &report(&reports, name).outcome.as_ref().unwrap_err().message;
    assert!(message(expected[1]).contains("different live unit of yours"));
    assert!(message(expected[0]).contains("every token in your hand"));
    assert!(message(expected[2]).contains("not being placed now"));
}

#[test]
fn tokens_are_revealed_in_order_and_only_numbered_ones() {
    let reports = run();
    let order = report(&reports, ORDER);
    assert!(order.outcome.is_ok(), "{}", summary(&reports));
    // Player 0's token 1 is on unit 1, player 1's token 1 on unit 3, then the second tokens.
    // The decoys, on units 2 and 5, are never revealed.
    assert_eq!(revealed(order), [1, 3, 0, 4]);
}

#[test]
fn each_player_is_dealt_numbered_tokens_and_a_decoy_and_places_them_all() {
    let reports = run();
    let order = report(&reports, ORDER);
    let dealt: Vec<(u8, Vec<u32>)> = order
        .events
        .iter()
        .filter_map(|e| match e {
            Event::TokensDealt { player, tokens } => {
                Some((player.0, tokens.iter().map(|t| t.0).collect()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(dealt, [(0, vec![1, 2, 0]), (1, vec![1, 2, 0])]);
    assert_eq!(count(order, |e| matches!(e, Event::TokenPlaced { .. })), 6);
}

#[test]
fn a_destroyed_unit_holding_a_token_loses_it_and_the_turn() {
    let reports = run();
    let lost = report(&reports, DESTROYED);
    assert!(lost.outcome.is_ok(), "{}", summary(&reports));
    let discarded: Vec<u32> = lost
        .events
        .iter()
        .filter_map(|e| match e {
            Event::TokensDiscarded(unit) => Some(unit.0),
            _ => None,
        })
        .collect();
    // Unit 0 is discarded when destroyed; the rest at the end of the round.
    assert_eq!(discarded[0], 0);
    assert_eq!(discarded.iter().filter(|u| **u == 0).count(), 1);
    assert_eq!(count(lost, |e| is_named(e, "turn_skipped")), 1);
    assert_eq!(count(lost, |e| is_named(e, "unit_removed")), 1);
    // Turn 3 was player 0's second token; it passes straight to player 1's turn 4.
    let turns: Vec<(u32, u8)> = lost
        .events
        .iter()
        .filter_map(|e| match e {
            Event::TurnStarted { turn, active } => Some((*turn, active.0)),
            _ => None,
        })
        .collect();
    assert_eq!(turns, [(1, 0), (2, 1), (3, 0), (4, 1)]);
    assert_eq!(revealed(lost), [1, 3, 4]);
}

#[test]
fn cards_are_cleared_when_the_round_ends() {
    let reports = run();
    let order = report(&reports, ORDER);
    // Every unit held one token, so all six are cleared, after the round ended.
    let ended = order.events.iter().position(|e| is_named(e, "round_ended"));
    let cleared = order
        .events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            Event::TokensDiscarded(_) => Some(i),
            _ => None,
        });
    let cleared: Vec<usize> = cleared.collect();
    assert_eq!(cleared.len(), 6);
    assert!(cleared.iter().all(|i| Some(*i) > ended));
    let state = state_after(&order.events, order.events.len());
    assert!(state.units.iter().all(|u| u.tokens.is_empty()));
}

#[test]
fn an_empty_tokens_slot_skips_placement_and_lets_players_choose() {
    let reports = run();
    let free = report(&reports, FREE);
    assert!(free.outcome.is_ok(), "{}", summary(&reports));
    let token_event = |e: &Event| {
        matches!(
            e,
            Event::TokensDealt { .. } | Event::TokenPlaced { .. } | Event::TokenRevealed { .. }
        )
    };
    assert_eq!(count(free, token_event), 0);
    assert_eq!(count(free, |e| matches!(e, Event::TurnStarted { .. })), 4);
}

#[test]
fn opponents_never_see_token_assignments_before_they_are_revealed() {
    let reports = run();
    let order = report(&reports, ORDER);
    let events = &order.events;
    let first_reveal = events
        .iter()
        .position(|e| matches!(e, Event::TokenRevealed { .. }))
        .unwrap();
    let before = &events[..first_reveal];
    let state = state_after(events, first_reveal);
    for viewer in [PlayerId(0), PlayerId(1)] {
        // The events: another player's placements arrive without their kinds.
        for event in visible_events(&state, viewer, before) {
            if let Event::TokenPlaced { unit, token } = event {
                assert_eq!(token.visibility, Visibility::Hidden);
                let own = state.units[unit.index()].owner == viewer;
                assert_eq!(
                    token.kind == REDACTED,
                    !own,
                    "unit {} for {viewer:?}",
                    unit.0
                );
            }
        }
        // The view: the same holds for the cards and for the hands.
        let view = PlayerView::of(&state, viewer);
        let tokens = view
            .state
            .units
            .iter()
            .enumerate()
            .flat_map(|(i, u)| u.tokens.iter().map(move |t| (u.owner == viewer, i, t.kind)));
        let mut seen = 0;
        for (own, unit, kind) in tokens {
            seen += 1;
            assert_eq!(kind == REDACTED, !own, "unit {unit} for {viewer:?}");
        }
        assert_eq!(seen, 6, "the opponent's tokens are counted, not named");
        assert!(view.state.players.iter().all(|p| p.hand.is_empty()));
    }
}

#[test]
fn a_revealed_token_is_visible_to_everyone() {
    let reports = run();
    let order = report(&reports, ORDER);
    let first_reveal = order
        .events
        .iter()
        .position(|e| matches!(e, Event::TokenRevealed { .. }))
        .unwrap();
    let state = state_after(&order.events, first_reveal + 1);
    let view = PlayerView::of(&state, PlayerId(1));
    // Unit 1 is player 0's, and its token 1 is now face up.
    let shown = &view.state.units[1].tokens;
    assert_eq!(shown.len(), 1);
    assert_eq!(
        (shown[0].kind.0, shown[0].visibility),
        (1, Visibility::Public)
    );
    // Another of player 0's cards is still hidden from player 1.
    assert_eq!(view.state.units[0].tokens[0].kind, REDACTED);
}

#[test]
fn running_the_tokens_pack_twice_gives_identical_events_and_hashes() {
    assert_eq!(run(), run());
}
