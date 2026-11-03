//! The built-in `action` command, through the `actions` fixture pack and `core:activation`.

use std::path::PathBuf;
use std::sync::OnceLock;

use strata_kernel::{ModuleId, SlotId, Value};
use strata_model::{Command, Event, PlayerId};
use strata_modules::native_modules;
use strata_rules::{
    ActivationRules, CommandHandler, ConfigSchema, Ctx, Halt, ModuleError, OrderingPolicy,
    Registrar, RuleModule,
};
use strata_session::pack_test::{PackTests, TestReport, summary};

/// Handles `start`: the player activates their first unit, with the default move-then-attack.
struct Start;

impl CommandHandler for Start {
    fn kind(&self) -> &str {
        "start"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, player: PlayerId, _: &Command) -> Result<(), Halt> {
        let unit = ctx.state().units.iter().position(|u| u.owner == player);
        let unit = unit.ok_or_else(|| Halt::reject("no unit"))?;
        let rules = ActivationRules {
            steps: ["move", "attack"].map(strata_kernel::Sym::new).to_vec(),
            budgets: Default::default(),
            ordering: OrderingPolicy::AllFiguresPerStep,
        };
        rules.begin(ctx, player, strata_model::UnitId(unit as u32))
    }
}

struct StartModule;

impl RuleModule for StartModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("test:start")
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
        r.command(Start);
        Ok(())
    }
}

fn run() -> Vec<TestReport> {
    let root = env!("CARGO_MANIFEST_DIR");
    let roots = [PathBuf::from(root).join("../../fixtures/packs/actions")];
    let mut modules = native_modules();
    modules.push(Box::new(StartModule));
    let loaded = PackTests::load(&roots).unwrap();
    let tests = modules.into_iter().try_fold(loaded, PackTests::with_native);
    tests.unwrap().run(None)
}

fn report<'a>(reports: &'a [TestReport], what: &str) -> &'a TestReport {
    reports.iter().find(|r| r.name == what).unwrap()
}

fn message(report: &TestReport) -> &str {
    &report.outcome.as_ref().unwrap_err().message
}

#[test]
fn a_squad_moves_every_figure_before_any_attacks() {
    let reports = run();
    let ran = report(&reports, "a squad moves every figure before any attacks");
    assert!(ran.outcome.is_ok(), "{}", summary(&reports));
    let spent = ran
        .events
        .iter()
        .filter(|e| matches!(e, Event::SlotsChanged { .. }))
        .count();
    assert_eq!(spent, 4);
}

#[test]
fn the_rules_refuse_actions_that_break_the_slots() {
    let reports = run();
    let early = report(
        &reports,
        "a figure cannot attack while another has yet to move",
    );
    assert!(message(early).contains("cannot spend those slots now"));
    let twice = report(&reports, "a figure cannot spend a slot twice");
    assert!(message(twice).contains("cannot spend those slots now"));
}

#[test]
fn targets_and_actors_are_checked_before_slots_are_spent() {
    let reports = run();
    let target = report(&reports, "an illegal target is refused");
    assert!(message(target).contains("illegal target"));
    let enemy = report(&reports, "a figure outside the active unit cannot act");
    assert!(message(enemy).contains("cannot act now"));
    let idle = report(&reports, "nothing can be done before a unit is activated");
    assert!(message(idle).contains("cannot act now"));
}

#[test]
fn running_the_pack_twice_gives_identical_events_and_hashes() {
    assert_eq!(run(), run());
}
