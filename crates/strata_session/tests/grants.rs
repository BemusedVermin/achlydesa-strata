//! Attached abilities feed stat queries (T-039 part 1). The fixture is `fixtures/pack_test/grants`.

use std::path::PathBuf;
use std::sync::OnceLock;

use strata_kernel::{DefId, ModuleId, SlotId, Sym, Value};
use strata_model::{AbilityOwner, Command, Event, PlayerId};
use strata_rules::{CommandHandler, ConfigSchema, Ctx, Halt, ModuleError, Registrar, RuleModule};
use strata_session::pack_test::{PackTests, TestReport, summary};

/// Handles `attach` and `detach`: `{ what, owner }` names an ability (`sharp`) or a relic's
/// first grant (`fury`) and the player it goes on.
struct Grants(&'static str);

impl CommandHandler for Grants {
    fn kind(&self) -> &str {
        self.0
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, command: &Command) -> Result<(), Halt> {
        let Command::Custom { payload, .. } = command else {
            return Err(Halt::reject("not a custom command"));
        };
        let owner = payload.get("owner").and_then(Value::as_int).unwrap_or(0);
        let owner = AbilityOwner::Player(PlayerId(owner as u8));
        let relic = payload.get("what").and_then(Value::as_str) == Ok("fury");
        let def = DefId(0);
        let params = if relic {
            [(Sym::new("grant"), Value::Int(0))].into()
        } else {
            Default::default()
        };
        match self.0 {
            "attach" => ctx.emit(Event::AbilityAttached { def, owner, params }),
            _ => ctx.emit(Event::AbilityDetached { def, owner }),
        }
    }
}

struct GrantModule;

impl RuleModule for GrantModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("t:grants")
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
        r.command(Grants("attach"));
        r.command(Grants("detach"));
        Ok(())
    }
}

fn run(filter: &str) -> Vec<TestReport> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pack_test/grants");
    let tests = PackTests::load(&[root]).unwrap();
    let tests = tests.with_native(Box::new(GrantModule)).unwrap();
    tests.run(Some(filter))
}

#[test]
fn an_attached_ability_changes_a_stat_until_it_is_detached() {
    let reports = run("an attached ability");
    assert!(reports[0].outcome.is_ok(), "{}", summary(&reports));
}

#[test]
fn a_relic_grant_applies_only_to_the_army_it_is_attached_to() {
    let reports = run("a relic grant");
    assert!(reports[0].outcome.is_ok(), "{}", summary(&reports));
}

#[test]
fn detaching_an_ability_that_is_not_attached_is_refused() {
    let reports = run("detaching an ability");
    let failure = reports[0].outcome.as_ref().unwrap_err();
    assert_eq!(failure.step, Some(1), "{}", summary(&reports));
}

#[test]
fn running_twice_gives_identical_hashes() {
    assert_eq!(run("an attached ability"), run("an attached ability"));
}
