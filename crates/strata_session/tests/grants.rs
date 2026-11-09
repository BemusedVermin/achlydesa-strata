//! Attached abilities feed stat queries (T-039 part 1). The fixture is `fixtures/pack_test/grants`.

use std::path::PathBuf;
use std::sync::OnceLock;

use strata_content::ContentRegistry;
use strata_kernel::{DefId, ModuleId, SlotId, Sym, Value};
use strata_model::{AbilityDef, AbilityKey, AbilityOwner, Command, Event, PlayerId, RelicDef};
use strata_model::{UnitDef, UnitId};
use strata_rules::{CommandHandler, ConfigSchema, Ctx, Halt, ModuleError, Registrar, RuleModule};
use strata_session::pack_test::{PackTests, TestReport, summary};

/// The fixture's packs.
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pack_test/grants")
}

/// The fixture's definitions the `attach` and `detach` commands name, looked up by name.
#[derive(Clone, Copy)]
struct Ids {
    sharp: DefId,
    fury: DefId,
    veil: DefId,
    veteran: DefId,
}

impl Ids {
    fn load() -> Self {
        let registry: ContentRegistry = strata_content::load(&[root()]).unwrap();
        let id = |found: Option<DefId>, name: &str| found.unwrap_or_else(|| panic!("no `{name}`"));
        Self {
            sharp: id(
                registry.lookup::<AbilityDef>("t", "sharp").map(|f| f.0),
                "sharp",
            ),
            fury: id(
                registry.lookup::<RelicDef>("t", "fury").map(|f| f.0),
                "fury",
            ),
            veil: id(
                registry.lookup::<RelicDef>("t", "veil").map(|f| f.0),
                "veil",
            ),
            veteran: id(
                registry.lookup::<UnitDef>("t", "test_veteran").map(|f| f.0),
                "veteran",
            ),
        }
    }
}

/// Handles `attach` and `detach`: `{ what, owner }` names an ability record (`sharp`) or a
/// relic's first grant (`fury`, `veil`) and the player it goes on, or the veteran's first
/// printed ability (`drill`) and the unit. Handles `probe` by emitting its payload as a `probe`
/// event.
struct Grants(&'static str, Ids);

impl CommandHandler for Grants {
    fn kind(&self) -> &str {
        self.0
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, command: &Command) -> Result<(), Halt> {
        let Command::Custom { payload, .. } = command else {
            return Err(Halt::reject("not a custom command"));
        };
        if self.0 == "probe" {
            let (id, payload) = (Sym::new("probe"), payload.clone());
            return ctx.emit(Event::Custom { id, payload });
        }
        let owner = payload.get("owner").and_then(Value::as_int).unwrap_or(0);
        let player = AbilityOwner::Player(PlayerId(owner as u8));
        let ids = self.1;
        let relic = |relic| AbilityKey::Relic { relic, index: 0 };
        let (def, owner) = match payload.get("what").and_then(Value::as_str) {
            Ok("fury") => (relic(ids.fury), player),
            Ok("veil") => (relic(ids.veil), player),
            Ok("drill") => {
                let unit = AbilityOwner::Unit(UnitId(owner as u32));
                let key = AbilityKey::Unit {
                    unit: ids.veteran,
                    index: 0,
                };
                (key, unit)
            }
            _ => (AbilityKey::Def(ids.sharp), player),
        };
        let params = Default::default();
        match self.0 {
            "attach" => ctx.emit(Event::AbilityAttached { def, owner, params }),
            _ => ctx.emit(Event::AbilityDetached { def, owner }),
        }
    }
}

struct GrantModule(Ids);

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
        r.command(Grants("attach", self.0));
        r.command(Grants("detach", self.0));
        r.command(Grants("probe", self.0));
        Ok(())
    }
}

fn run(filter: &str) -> Vec<TestReport> {
    let tests = PackTests::load(&[root()]).unwrap();
    let tests = tests
        .with_native(Box::new(GrantModule(Ids::load())))
        .unwrap();
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
fn a_printed_unit_ability_applies_with_no_manual_attach() {
    let reports = run("a printed unit ability");
    assert!(reports[0].outcome.is_ok(), "{}", summary(&reports));
}

#[test]
fn a_pair_query_gathers_the_context_figures_selected_modifiers() {
    let reports = run("a pair query");
    assert!(reports[0].outcome.is_ok(), "{}", summary(&reports));
}

#[test]
fn a_context_side_veto_stops_others_targeting_its_owner_not_the_owner() {
    let reports = run("a context-side veto stops");
    assert_eq!(reports.len(), 1, "{}", summary(&reports));
    assert!(reports[0].outcome.is_ok(), "{}", summary(&reports));
}

#[test]
fn a_context_side_veto_in_mirror_armies_applies_only_to_its_owner() {
    let reports = run("mirror armies");
    assert_eq!(reports.len(), 1, "{}", summary(&reports));
    assert!(reports[0].outcome.is_ok(), "{}", summary(&reports));
}

#[test]
fn a_player_owned_grant_on_the_context_side_shields_that_army() {
    let reports = run("a player-owned grant");
    assert_eq!(reports.len(), 1, "{}", summary(&reports));
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
    assert_eq!(run("a printed unit ability"), run("a printed unit ability"));
    assert_eq!(run("a pair query"), run("a pair query"));
    assert_eq!(run("a context-side veto"), run("a context-side veto"));
}
