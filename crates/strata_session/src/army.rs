//! Army list legality: a list checked against a format's and a scenario's army rules, through
//! the ruleset's `army` slot (design §11.3, §11.4).

use strata_content::ContentRegistry;
use strata_kernel::{Sym, Value};
use strata_lua::{PackSource, RuntimeLua};
use strata_model::{
    ArmyList, FactionDef, FootprintDef, FormatDef, Logic, ScenarioDef, ScriptRef, StatDef, UnitDef,
};
use strata_rules::{
    ArmyRules, ArmyUnit, ArmyViolation, EvalCtx, LogicError, RulesRuntime, ScriptCaller,
};

use crate::pack_test::host::find;
use crate::pack_test::run::Rulesets;
use crate::pack_test::{PackTestError, PackTests};

/// The stat that prices a unit.
const POINTS: &str = "points";

/// The entries of `list` with what the army rules need to know about each unit, its names
/// looked up from `pack`. Points are the printed `points` stat, never stored in the list, so a
/// balance patch reprices saved lists.
pub fn army_units(
    registry: &ContentRegistry,
    pack: &str,
    list: &ArmyList,
) -> Result<Vec<ArmyUnit>, String> {
    let points = find::<StatDef>(registry, pack, POINTS).map(|(id, _)| id);
    let mut units = Vec::new();
    for entry in &list.units {
        let name = entry.unit.as_str();
        let (id, def) = find::<UnitDef>(registry, pack, name)
            .ok_or_else(|| format!("army: unknown unit `{name}`"))?;
        let printed = points.and_then(|p| def.stats.get(&p).copied());
        let faction = def.faction.and_then(|f| registry.name::<FactionDef>(f));
        let mut footprint = 0;
        for figure in &def.figures {
            let shape = registry.get::<FootprintDef>(figure.footprint);
            let cells = shape.map_or(1, |s| s.cells.len()) as u32;
            footprint += cells * u32::from(figure.count);
        }
        units.push(ArmyUnit {
            unit: Sym::new(registry.name::<UnitDef>(id).unwrap_or(name)),
            copies: entry.copies,
            points: i64::from(printed.unwrap_or(0)),
            rarity: def.rarity.clone(),
            faction: faction.map(Sym::new),
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

/// Checks `list` against the rules of `format` and of `scenario`, both sets at once, with the
/// `army` module of `rt`. Returns every violation with the rule that produced it, the format's
/// first. Names in the list are looked up from `pack`; `packs` supply any rule scripts.
pub fn validate_army(
    registry: &ContentRegistry,
    packs: &[PackSource],
    rt: &RulesRuntime,
    pack: &str,
    list: &ArmyList,
    format: Option<&FormatDef>,
    scenario: Option<&ScenarioDef>,
) -> Result<Vec<ArmyViolation>, String> {
    let rules = rt.service::<dyn ArmyRules>();
    let rules = rules.ok_or("the ruleset has no `army` module")?;
    let army = army_units(registry, pack, list)?;
    let lists = format.map(|f| &f.army).into_iter();
    let lists = lists.chain(scenario.map(|s| &s.army));
    let scripted = lists.flatten().any(|l| matches!(l, Logic::Script(_)));
    let lua = if scripted {
        Some(RuntimeLua::new(&registry.scripts(), packs).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let found = strata_rules::army::validate(rules, &Scripts(lua), &army, format, scenario);
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
        self.army_check("", ruleset, list, format, scenario)
            .map_err(PackTestError)
    }

    /// [`PackTests::validate_army`], with names looked up from `pack` first.
    pub(crate) fn army_check(
        &self,
        pack: &str,
        ruleset: &str,
        list: &str,
        format: Option<&str>,
        scenario: Option<&str>,
    ) -> Result<Vec<ArmyViolation>, String> {
        let registry = &self.registry;
        let rulesets = Rulesets { tests: self, pack };
        let rt = strata_rules::resolve(ruleset, &rulesets, &self.catalog);
        let rt = rt.map_err(|e| e.to_string())?;
        let (_, army) = find::<ArmyList>(registry, pack, list)
            .ok_or_else(|| format!("unknown army list `{list}`"))?;
        let format = match format {
            Some(name) => Some(
                find::<FormatDef>(registry, pack, name)
                    .ok_or_else(|| format!("unknown format `{name}`"))?
                    .1,
            ),
            None => None,
        };
        let scenario = match scenario {
            Some(name) => Some(
                find::<ScenarioDef>(registry, pack, name)
                    .ok_or_else(|| format!("unknown scenario `{name}`"))?
                    .1,
            ),
            None => None,
        };
        validate_army(registry, &self.packs, &rt, pack, army, format, scenario)
    }
}
