use std::path::PathBuf;

use strata_kernel::{DefId, ModuleId, SlotId, Value};
use strata_model::{Command, Event, FigureId, PlayerId};
use strata_rules::{CommandHandler, ConfigSchema, Ctx, Halt, ModuleError, Registrar, RuleModule};
use strata_session::pack_test::{PackTests, TestReport, summary};

/// The packs in `fixtures/pack_test/<name>`.
fn fixture(name: &str) -> Vec<PathBuf> {
    let root = env!("CARGO_MANIFEST_DIR");
    vec![
        PathBuf::from(root)
            .join("../../fixtures/pack_test")
            .join(name),
    ]
}

/// Handles `hit` and `kill`: each acts on the figure its payload names, or figure 0.
struct Driver(&'static str);

impl CommandHandler for Driver {
    fn kind(&self) -> &str {
        self.0
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, command: &Command) -> Result<(), Halt> {
        let Command::Custom { payload, .. } = command else {
            return Err(Halt::reject("not a custom command"));
        };
        let figure = FigureId(payload.as_int().unwrap_or(0) as u32);
        match self.0 {
            "hit" => ctx.emit(Event::WoundsApplied {
                figure,
                wound: DefId(0),
                amount: 1,
            }),
            _ => ctx.emit(Event::FigureDestroyed(figure)),
        }
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
        r.command(Driver("hit"));
        r.command(Driver("kill"));
        Ok(())
    }
}

fn run(filter: Option<&str>) -> Vec<TestReport> {
    let tests = PackTests::load(&fixture("full")).unwrap();
    tests.with_native(Box::new(Commands)).unwrap().run(filter)
}

fn named<'a>(reports: &'a [TestReport], name: &str) -> &'a TestReport {
    reports.iter().find(|r| r.name == name).unwrap()
}

fn assert_passes(name: &str) {
    let reports = run(Some(name));
    assert_eq!(reports.len(), 1, "{}", summary(&reports));
    assert!(reports[0].outcome.is_ok(), "{}", summary(&reports));
}

#[test]
fn a_passing_test_runs_modifiers_native_commands_scripts_and_a_map() {
    assert_passes("a modifier raises");
}

#[test]
fn a_handle_may_leave_out_the_figure_name() {
    assert_passes("a handle may");
}

#[test]
fn a_script_selects_enemies_with_a_native_descriptor() {
    assert_passes("a script selects enemies");
}

#[test]
fn rigged_dice_survive_answering_a_decision() {
    assert_passes("rigged dice survive");
}

#[test]
fn a_destroyed_figure_is_reported() {
    assert_passes("a destroyed figure");
}

#[test]
fn failures_name_the_test_the_step_and_expected_versus_actual() {
    let reports = run(None);
    let wrong = named(&reports, "wrong stat").to_string();
    assert_eq!(
        wrong,
        "FAIL wrong stat (step 2): p0.soldier.f#1 defense: expected 9, got 5"
    );
    let passed = reports.iter().filter(|r| r.outcome.is_ok()).count();
    assert_eq!(passed, 5, "{}", summary(&reports));
    insta::assert_snapshot!(summary(&reports));
}

#[test]
fn a_script_decision_with_no_valid_answer_rejects_the_command_instead_of_waiting() {
    let reports = run(Some("no valid answer"));
    assert_eq!(reports.len(), 1, "{}", summary(&reports));
    assert_eq!(
        reports[0].to_string(),
        "FAIL a decision with no valid answer is refused (step 1): `hit` was rejected: \
         ctx.choose: the decision offers no figures"
    );
}

#[test]
fn the_filter_selects_tests_by_name() {
    let reports = run(Some("wrong"));
    assert_eq!(reports.len(), 7);
    assert!(reports.iter().all(|r| r.name.contains("wrong")));
}

#[test]
fn running_twice_gives_identical_reports_and_state_hashes() {
    let (first, second) = (run(None), run(None));
    assert_eq!(first, second);
    let hashes = named(&first, "rigged dice survive answering a decision");
    assert_eq!(hashes.outcome.as_ref().unwrap().len(), 1);
}

#[test]
fn a_pack_that_does_not_load_is_an_error_not_a_failed_test() {
    let dir = tempfile::tempdir().unwrap();
    let pack = dir.path().join("t");
    std::fs::create_dir_all(&pack).unwrap();
    let manifest = "return { id = 't', version = '1.0.0', name = 't', engine = '>=0.1, <0.2' }";
    std::fs::write(pack.join("pack.lua"), manifest).unwrap();
    std::fs::write(pack.join("data.lua"), "stat { id = 'x' }").unwrap();
    let error = PackTests::load(&[dir.path().to_owned()]).err().unwrap();
    assert!(error.0.contains("data.lua"), "{error}");
}
