//! Army list legality: a list checked against a format's and a scenario's army rules, through
//! the ruleset's `army` slot (design §11.3, §11.4).

use strata_content::ContentRegistry;
use strata_kernel::{DefId, Sym, Value};
use strata_lua::{Def, PackSource, RuntimeLua};
use strata_model::{
    ArmyList, FactionDef, FootprintDef, FormatDef, Logic, ScenarioDef, ScriptRef, StatDef, UnitDef,
};
use strata_rules::{
    ArmyNames, ArmyRules, ArmyUnit, ArmyViolation, EvalCtx, LogicError, NameKind, RuleOrigin,
    RulesRuntime, ScriptCaller,
};

use crate::pack_test::host::find;
use crate::pack_test::run::Rulesets;
use crate::pack_test::{PackTestError, PackTests};

/// The stat that prices a unit.
const POINTS: &str = "points";

/// The printed `points` stat of `def`, with names looked up from `pack`: what one copy costs,
/// with no modifier applied, and 0 when the unit prints none. `None` when the packs define no
/// `points` stat. Army rules and the victory tiebreaks both price units through this.
pub(crate) fn printed_points(registry: &ContentRegistry, pack: &str, def: &UnitDef) -> Option<i64> {
    let (stat, _) = find::<StatDef>(registry, pack, POINTS)?;
    Some(def.stats.get(&stat).copied().map_or(0, i64::from))
}

/// The pack that defines the `T` with this ID: the part of its full name before `:`.
fn pack_of<T: Def>(registry: &ContentRegistry, id: DefId) -> &str {
    let name = registry.name::<T>(id);
    name.and_then(|n| n.split_once(':'))
        .map_or("", |(pack, _)| pack)
}

/// The entries of `list` with what the army rules need to know about each unit, its names
/// looked up from `pack`. Points are the printed `points` stat, never stored in the list, so a
/// balance patch reprices saved lists.
pub fn army_units(
    registry: &ContentRegistry,
    pack: &str,
    list: &ArmyList,
) -> Result<Vec<ArmyUnit>, String> {
    let mut units = Vec::new();
    for entry in &list.units {
        let name = entry.unit.as_str();
        let (id, def) = find::<UnitDef>(registry, pack, name)
            .ok_or_else(|| format!("army: unknown unit `{name}`"))?;
        let faction = def.faction.and_then(|f| registry.name::<FactionDef>(f));
        let mut footprint = 0;
        for figure in &def.figures {
            let shape = registry.get::<FootprintDef>(figure.footprint);
            let cells = shape.map_or(1, |s| s.cells.len()) as u32;
            footprint += cells * u32::from(figure.count);
        }
        units.push(ArmyUnit {
            unit: Sym::new(registry.name::<UnitDef>(id).unwrap_or(name)),
            def: id,
            copies: entry.copies,
            points: printed_points(registry, pack, def),
            rarity: def.rarity.clone(),
            faction: faction.map(Sym::new),
            faction_def: def.faction,
            footprint,
        });
    }
    Ok(units)
}

/// Calls army rules written as Lua functions with the army as their only argument.
struct Scripts(Option<RuntimeLua>);

impl ScriptCaller for Scripts {
    fn call(&self, script: &ScriptRef, cx: &EvalCtx<'_>) -> Result<Value, LogicError> {
        let lua = self.0.as_ref();
        let lua = lua.ok_or_else(|| LogicError("no script state for army rules".into()))?;
        let found = lua.call_script(script, std::slice::from_ref(cx.subject));
        found.map_err(|e| LogicError(e.to_string()))
    }
}

/// Looks up the names a rule lists from the pack that defines its format or scenario.
struct Names<'a> {
    registry: &'a ContentRegistry,
    format: &'a str,
    scenario: &'a str,
}

impl ArmyNames for Names<'_> {
    fn find(&self, origin: RuleOrigin, kind: NameKind, name: &str) -> Option<DefId> {
        let pack = match origin {
            RuleOrigin::Format => self.format,
            RuleOrigin::Scenario => self.scenario,
        };
        match kind {
            NameKind::Unit => find::<UnitDef>(self.registry, pack, name).map(|(id, _)| id),
            NameKind::Faction => find::<FactionDef>(self.registry, pack, name).map(|(id, _)| id),
        }
    }
}

/// Checks `list` against the rules of the format `format` and of the scenario `scenario`, both
/// sets at once, with the `army` module of `rt`. Returns every violation with the rule that
/// produced it, the format's first. Names in the list are looked up from `pack`; names in a rule
/// from the pack that defines its format or scenario. `packs` supply any rule scripts.
pub fn validate_army(
    registry: &ContentRegistry,
    packs: &[PackSource],
    rt: &RulesRuntime,
    pack: &str,
    list: &ArmyList,
    format: Option<DefId>,
    scenario: Option<DefId>,
) -> Result<Vec<ArmyViolation>, String> {
    let rules = rt.service::<dyn ArmyRules>();
    let rules = rules.ok_or("the ruleset has no `army` module")?;
    let army = army_units(registry, pack, list)?;
    let format_def = format.and_then(|id| registry.get::<FormatDef>(id));
    let scenario_def = scenario.and_then(|id| registry.get::<ScenarioDef>(id));
    let names = Names {
        registry,
        format: format.map_or("", |id| pack_of::<FormatDef>(registry, id)),
        scenario: scenario.map_or("", |id| pack_of::<ScenarioDef>(registry, id)),
    };
    let lists = format_def.map(|f| &f.army).into_iter();
    let lists = lists.chain(scenario_def.map(|s| &s.army));
    let scripted = lists.flatten().any(|l| matches!(l, Logic::Script(_)));
    let lua = if scripted {
        Some(RuntimeLua::new(&registry.scripts(), packs).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let scripts = Scripts(lua);
    let found =
        strata_rules::army::validate(rules, &scripts, &names, &army, format_def, scenario_def);
    found.map_err(|e| e.to_string())
}

impl PackTests {
    /// Checks the army list `list` against the format and scenario named, under the ruleset
    /// `ruleset`'s `army` module. Names are looked up as written, then in any pack.
    pub fn validate_army(
        &self,
        ruleset: &str,
        list: &str,
        format: Option<&str>,
        scenario: Option<&str>,
    ) -> Result<Vec<ArmyViolation>, PackTestError> {
        let rulesets = Rulesets {
            tests: self,
            pack: "",
        };
        let rt = strata_rules::resolve(ruleset, &rulesets, &self.catalog);
        let rt = rt.map_err(|e| PackTestError(e.to_string()))?;
        self.army_check("", &rt, list, format, scenario)
            .map_err(PackTestError)
    }

    /// [`PackTests::validate_army`] under the resolved rules `rt`, with names looked up from
    /// `pack` first.
    pub(crate) fn army_check(
        &self,
        pack: &str,
        rt: &RulesRuntime,
        list: &str,
        format: Option<&str>,
        scenario: Option<&str>,
    ) -> Result<Vec<ArmyViolation>, String> {
        let registry = &self.registry;
        let (_, army) = find::<ArmyList>(registry, pack, list)
            .ok_or_else(|| format!("unknown army list `{list}`"))?;
        let format = format.map(|n| {
            let found = find::<FormatDef>(registry, pack, n).map(|(id, _)| id);
            found.ok_or_else(|| format!("unknown format `{n}`"))
        });
        let scenario = scenario.map(|n| {
            let found = find::<ScenarioDef>(registry, pack, n).map(|(id, _)| id);
            found.ok_or_else(|| format!("unknown scenario `{n}`"))
        });
        let (format, scenario) = (format.transpose()?, scenario.transpose()?);
        validate_army(registry, &self.packs, rt, pack, army, format, scenario)
    }
}
