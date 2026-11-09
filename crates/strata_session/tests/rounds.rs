//! `core:rounds`, through the `rounds` fixture pack.

use std::path::PathBuf;
use std::sync::OnceLock;

use strata_kernel::{ModuleId, SlotId, Sym, Value};
use strata_model::{Command, Descriptor, Event, FigureId, Logic, PlayerId, TriggerDef, UnitId};
use strata_modules::native_modules;
use strata_rules::{
    CommandHandler, ConfigSchema, Ctx, Halt, ModuleError, NativeEnv, NativeLogic, Registrar,
    RuleModule,
};
use strata_session::pack_test::{PackTests, TestReport, summary};

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

/// Handles `mark { figure, rounds }` by passing it on as a `mark` event, since only native logic
/// can look markers up.
struct MarkHandler;

impl CommandHandler for MarkHandler {
    fn kind(&self) -> &str {
        "mark"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, command: &Command) -> Result<(), Halt> {
        let Command::Custom { payload, .. } = command else {
            return Err(Halt::reject("not a custom command"));
        };
        ctx.emit(Event::Custom {
            id: Sym::new("mark"),
            payload: payload.clone(),
        })
    }
}

/// Puts one `stunned` marker on the figure, lasting `rounds` rounds the way `eff.add_marker`
/// counts them: the current round is the first.
struct Stun;

impl NativeLogic for Stun {
    fn run(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, event: &Event) -> Result<(), Halt> {
        let Event::Custom { payload, .. } = event else {
            return Ok(());
        };
        let int = |name| payload.get(name).and_then(Value::as_int).ok();
        let figure = FigureId(int("figure").ok_or_else(|| Halt::reject("no figure"))? as u32);
        let round = ctx.state().flow.round;
        let expiry = int("rounds").map(|n| round + n as u32 - 1);
        let marker = env
            .marker("stunned")
            .ok_or_else(|| Halt::reject("no marker"))?;
        ctx.emit(Event::MarkerChanged {
            figure,
            marker,
            delta: 1,
            source: None,
            expiry,
        })
    }
}

struct Mark;

impl RuleModule for Mark {
    fn id(&self) -> ModuleId {
        ModuleId::new("test:mark")
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
        r.command(MarkHandler);
        r.native("test:mark.stun", Box::new(Stun));
        let call = [(Sym::new("native"), Value::Str(Sym::new("test:mark.stun")))];
        r.trigger(TriggerDef {
            event: Sym::new("mark"),
            filter: None,
            limit: None,
            effect: Logic::Native(Descriptor(Value::Map(call.into()))),
        });
        Ok(())
    }
}

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/rounds")];
    let mut modules = native_modules();
    modules.push(Box::new(Pass));
    modules.push(Box::new(Mark));
    let loaded = PackTests::load(&roots).unwrap();
    let tests = modules.into_iter().try_fold(loaded, PackTests::with_native);
    tests.unwrap().run(None)
}

fn report<'a>(reports: &'a [TestReport], what: &str) -> &'a TestReport {
    reports.iter().find(|r| r.name == what).unwrap()
}

/// Who starts each turn, in order, across every command of the test.
fn turns(report: &TestReport) -> Vec<(u32, u8)> {
    let started = report.events.iter().filter_map(|e| match e {
        Event::TurnStarted { turn, active } => Some((*turn, active.0)),
        _ => None,
    });
    started.collect()
}

fn count(report: &TestReport, what: impl Fn(&Event) -> bool) -> usize {
    report.events.iter().filter(|e| what(e)).count()
}

fn is_named(event: &Event, name: &str) -> bool {
    matches!(event, Event::Custom { id, .. } if id.as_str() == name)
}

#[test]
fn every_pack_test_passes_except_the_two_that_expect_a_refusal() {
    let reports = run();
    let failed: Vec<&str> = reports
        .iter()
        .filter(|r| r.outcome.is_err())
        .map(|r| r.name.as_str())
        .collect();
    let expected = [
        "a second round cannot start mid-round",
        "a player cannot pass out of turn",
    ];
    assert_eq!(failed, expected, "{}", summary(&reports));
    let message = |name| &report(&reports, name).outcome.as_ref().unwrap_err().message;
    assert!(message(expected[1]).contains("not your turn"));
    assert!(message(expected[0]).contains("already under way"));
}

#[test]
fn the_higher_initiative_roll_takes_the_first_turn() {
    let reports = run();
    let high = report(&reports, "the higher initiative roll goes first");
    assert_eq!(turns(high), [(1, 0)]);
    let low = report(&reports, "the lower initiative roll goes second");
    assert_eq!(turns(low), [(1, 1)]);
}

#[test]
fn a_tie_is_rerolled_between_the_tied_players_only() {
    let reports = run();
    let tie = report(&reports, "a tie is rerolled by the tied players");
    // 2 and 2 tie; the reroll gives player 0 a 1 and player 1 a 3.
    assert_eq!(turns(tie), [(1, 1)]);
    assert_eq!(count(tie, |e| matches!(e, Event::DiceRolled { .. })), 4);
}

#[test]
fn a_kept_tie_goes_to_last_rounds_first_player() {
    let reports = run();
    let keep = report(&reports, "a kept tie favours the previous first player");
    assert!(keep.outcome.is_ok(), "{}", summary(&reports));
    // Round 1: player 1 won initiative. Round 2 ties, so player 1 stays first.
    assert_eq!(turns(keep), [(1, 1), (2, 0), (1, 1)]);
    let rolls = count(keep, |e| matches!(e, Event::DiceRolled { .. }));
    assert_eq!(rolls, 4, "a kept tie rolls nothing extra");
}

#[test]
fn turns_alternate_for_turns_per_round_each_then_the_round_ends() {
    let reports = run();
    let long = report(&reports, "players alternate turns until the round ends");
    assert!(long.outcome.is_ok(), "{}", summary(&reports));
    assert_eq!(turns(long), [(1, 0), (2, 1), (3, 0), (4, 1)]);
    assert_eq!(count(long, |e| is_named(e, "round_ended")), 1);
    // initiative, turns, round_end and round.
    assert_eq!(count(long, |e| matches!(e, Event::PhaseExited(_))), 4);
}

#[test]
fn the_round_counter_advances_each_round() {
    let reports = run();
    let keep = report(&reports, "a kept tie favours the previous first player");
    let rounds: Vec<u32> = keep
        .events
        .iter()
        .filter_map(|e| match e {
            Event::RoundStarted(n) => Some(*n),
            _ => None,
        })
        .collect();
    assert_eq!(rounds, [1, 2]);
}

#[test]
fn running_the_rounds_pack_twice_gives_identical_events_and_hashes() {
    assert_eq!(run(), run());
}

/// Each marker change in the test as `(round, round_ended already, delta, expiry)`.
fn marker_changes(report: &TestReport) -> Vec<(u32, bool, i32, Option<u32>)> {
    let (mut round, mut ended) = (0, false);
    let mut changes = Vec::new();
    for event in &report.events {
        match event {
            Event::RoundStarted(n) => (round, ended) = (*n, false),
            e if is_named(e, "round_ended") => ended = true,
            Event::MarkerChanged { delta, expiry, .. } => {
                changes.push((round, ended, *delta, *expiry));
            }
            _ => {}
        }
    }
    changes
}

#[test]
fn a_marker_lasts_through_its_last_round_and_lapses_when_that_round_ends() {
    let reports = run();
    let expire = report(
        &reports,
        "markers with a duration lapse when their last round ends",
    );
    assert!(expire.outcome.is_ok(), "{}", summary(&reports));
    assert_eq!(count(expire, |e| is_named(e, "round_ended")), 3);
    let changes = marker_changes(expire);
    let added: Vec<_> = changes.iter().filter(|c| c.2 > 0).copied().collect();
    // Placed during round 1: `rounds = 1` ends with round 1, `rounds = 2` with round 2.
    assert_eq!(
        added,
        [
            (1, false, 1, Some(1)),
            (1, false, 1, Some(2)),
            (1, false, 1, None)
        ]
    );
    let removed: Vec<_> = changes.iter().filter(|c| c.2 < 0).copied().collect();
    // Each is removed only once its last round has ended, and the marker without a duration
    // is still there after round 3.
    assert_eq!(removed, [(1, true, -1, Some(1)), (2, true, -1, Some(2))]);
}

#[test]
fn marker_expiry_gives_the_same_hashes_on_every_run() {
    let name = "markers with a duration lapse when their last round ends";
    let hashes = || report(&run(), name).outcome.clone().unwrap();
    let first = hashes();
    assert!(!first.is_empty());
    assert_eq!(first, hashes());
}
