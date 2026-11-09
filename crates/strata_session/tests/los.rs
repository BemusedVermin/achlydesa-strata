//! Pack tests for line of sight: the `in_los` selector against the `core:los` module.

use std::path::PathBuf;

use strata_kernel::{DefId, ModuleId, SlotId, Value};
use strata_model::{Command, Event, FigureId, PlayerId};
use strata_modules::native_modules;
use strata_rules::{CommandHandler, ConfigSchema, Ctx, Halt, ModuleError, Registrar, RuleModule};
use strata_session::pack_test::{PackTests, TestReport, summary};

/// Handles `hit`: wounds the figure its payload names.
struct Hit;

impl CommandHandler for Hit {
    fn kind(&self) -> &str {
        "hit"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, command: &Command) -> Result<(), Halt> {
        let Command::Custom { payload, .. } = command else {
            return Err(Halt::reject("not a custom command"));
        };
        ctx.emit(Event::WoundsApplied {
            figure: FigureId(payload.as_int().unwrap_or(0) as u32),
            wound: DefId(0),
            amount: 1,
        })
    }
}

struct Commands;

impl RuleModule for Commands {
    fn id(&self) -> ModuleId {
        ModuleId::new("t:driver")
    }

    fn slot(&self) -> Option<SlotId> {
        None
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        static EMPTY: std::sync::OnceLock<ConfigSchema> = std::sync::OnceLock::new();
        EMPTY.get_or_init(ConfigSchema::default)
    }

    fn register(&self, r: &mut Registrar, _: &Value) -> Result<(), ModuleError> {
        r.command(Hit);
        Ok(())
    }
}

fn run() -> Vec<TestReport> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pack_test/los");
    let loaded = PackTests::load(&[root]).unwrap();
    let modules = native_modules().into_iter();
    let tests = modules.fold(loaded, |t, m| t.with_native(m).unwrap());
    tests.with_native(Box::new(Commands)).unwrap().run(None)
}

#[test]
fn walls_bridges_cover_and_figures_decide_who_is_in_line_of_sight() {
    let reports = run();
    assert_eq!(reports.len(), 11, "{}", summary(&reports));
    assert!(
        reports.iter().all(|r| r.outcome.is_ok()),
        "{}",
        summary(&reports)
    );
}

#[test]
fn a_two_space_figure_is_seen_and_blocks_at_each_cells_own_level() {
    let reports = run();
    let names = [
        "a rear cell a level up is seen over a low wall",
        "a rear cell on the anchor's level stays behind a low wall",
        "the lane is open end to end",
        "a rear cell a level below its anchor blocks sight past it",
    ];
    for what in names {
        let found = reports.iter().find(|r| r.name == what);
        let ran = found.unwrap_or_else(|| panic!("no test `{what}`:\n{}", summary(&reports)));
        assert!(ran.outcome.is_ok(), "{what}: {}", summary(&reports));
    }
}

#[test]
fn running_the_line_of_sight_tests_twice_gives_identical_hashes() {
    assert_eq!(run(), run());
}
