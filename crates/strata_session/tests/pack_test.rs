use std::path::Path;

use strata_kernel::{DefId, ModuleId, SlotId, Value};
use strata_model::{Command, Event, FigureId, PlayerId};
use strata_rules::{CommandHandler, ConfigSchema, Ctx, Halt, ModuleError, Registrar, RuleModule};
use strata_session::pack_test::{PackTests, TestReport, summary};

const DATA: &str = r#"
stat { id = 'defense', name = 'Defense', min = 0, max = 9 }
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'soldier', name = 'Soldier', kind = 'squad', rarity = 'common',
  stats = { defense = 4 },
  figures = {{
    id = 'f', count = 2, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}
unit {
  id = 'dummy', name = 'Dummy', kind = 'squad', rarity = 'common',
  stats = { defense = 1 },
  figures = {{
    id = 'd', count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}
rule_module {
  id = 'rules',
  register = function(r)
    r.modifier { query = 'defense', layer = 'add', value = function() return 1 end }
    r.trigger { event = 'wounds_applied', effect = function(ctx) ctx.emit('reacted', 1) end }
  end,
}
ruleset { id = 'rs', name = 'Test', addons = {{ id = 't:driver' }, { id = 't:rules' }} }

local function soldiers()
  return {{ unit = 'soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }, { q = 1, r = 0, z = 1 }} }}
end
local function dummy()
  return {{ unit = 'dummy', player = 0, at = {{ q = 0, r = 0, z = 1 }} }}
end

test {
  id = 'passes', name = 'a modifier raises a stat and a script reacts to an event',
  ruleset = 'rs', place = soldiers(),
  steps = {
    expect.stat('p0.soldier.f#2', 'defense', 5),
    act.command(0, 'hit', 1),
    expect.event('wounds_applied'),
    expect.event('reacted'),
  },
}
test {
  id = 'bare_unit_handle', name = 'a handle may leave out the figure name',
  ruleset = 'rs', place = dummy(),
  steps = { expect.stat('p0.dummy#1', 'defense', 2) },
}
test {
  id = 'wrong_stat', name = 'wrong stat', ruleset = 'rs', place = soldiers(),
  steps = { expect.stat('p0.soldier.f#1', 'defense', 5), expect.stat('p0.soldier.f#1', 'defense', 9) },
}
test {
  id = 'wrong_event', name = 'wrong event', ruleset = 'rs', place = soldiers(),
  steps = { act.command(0, 'hit'), expect.event('figure_destroyed') },
}
test {
  id = 'bad_handle', name = 'bad handle', ruleset = 'rs', place = dummy(),
  steps = { expect.stat('p0.dummy', 'defense', 2) },
}
test {
  id = 'missing_figure', name = 'missing figure', ruleset = 'rs', place = dummy(),
  steps = { expect.stat('p1.dummy#1', 'defense', 2) },
}
test {
  id = 'unknown_command', name = 'unknown command', ruleset = 'rs', place = dummy(),
  steps = { act.command(0, 'nope') },
}
test {
  id = 'unknown_unit', name = 'unknown unit', ruleset = 'rs',
  place = {{ unit = 'ghost', player = 0, at = {{ q = 0, r = 0, z = 1 }} }},
}
test {
  id = 'wrong_count', name = 'wrong figure count', ruleset = 'rs',
  place = {{ unit = 'soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }} }},
}
test { id = 'no_ruleset', name = 'no ruleset' }
test { id = 'unknown_ruleset', name = 'unknown ruleset', ruleset = 'nope' }
test { id = 'unknown_map', name = 'unknown map', ruleset = 'rs', map = 'nope' }
"#;

/// Wounds the figure its payload names, or figure 0, when it receives `hit`.
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

struct Driver;

impl RuleModule for Driver {
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

fn write(root: &Path, pack: &str, data: &str) {
    let dir = root.join(pack);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = format!(
        "return {{ id = '{pack}', version = '1.0.0', name = '{pack}', engine = '>=0.1, <0.2' }}"
    );
    std::fs::write(dir.join("pack.lua"), manifest).unwrap();
    std::fs::write(dir.join("data.lua"), data).unwrap();
}

fn run(filter: Option<&str>) -> Vec<TestReport> {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "t", DATA);
    let tests = PackTests::load(&[dir.path().to_owned()]).unwrap();
    tests.with_native(Box::new(Driver)).unwrap().run(filter)
}

fn named<'a>(reports: &'a [TestReport], name: &str) -> &'a TestReport {
    reports.iter().find(|r| r.name == name).unwrap()
}

#[test]
fn a_passing_test_runs_modifiers_native_commands_and_scripts() {
    let reports = run(Some("a modifier raises"));
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].outcome, Ok(()), "{}", summary(&reports));
}

#[test]
fn a_handle_may_leave_out_the_figure_name() {
    let reports = run(Some("a handle may"));
    assert_eq!(reports[0].outcome, Ok(()), "{}", summary(&reports));
}

#[test]
fn failures_name_the_test_the_step_and_expected_versus_actual() {
    let reports = run(None);
    let failed: Vec<_> = reports.iter().filter(|r| r.outcome.is_err()).collect();
    assert_eq!(failed.len(), reports.len() - 2);
    let wrong = named(&reports, "wrong stat").to_string();
    assert_eq!(
        wrong,
        "FAIL wrong stat (step 2): p0.soldier.f#1 defense: expected 9, got 5"
    );
    insta::assert_snapshot!(summary(&reports));
}

#[test]
fn the_filter_selects_tests_by_name() {
    let reports = run(Some("wrong"));
    assert_eq!(reports.len(), 3);
    assert!(reports.iter().all(|r| r.name.contains("wrong")));
}

#[test]
fn running_twice_gives_identical_reports() {
    assert_eq!(run(None), run(None));
}

#[test]
fn a_pack_that_does_not_load_is_an_error_not_a_failed_test() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "t", "stat { id = 'x' }");
    let error = PackTests::load(&[dir.path().to_owned()]).err().unwrap();
    assert!(error.0.contains("data.lua"), "{error}");
}
