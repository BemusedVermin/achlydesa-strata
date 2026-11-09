//! `core:army`: army legality, from points, uniqueness, copies, bans, factions and footprint
//! (design §6.1, §11.3, §11.4).
//!
//! The module provides [`ArmyRules`] for the native rules the `army_rule.*` constructors write:
//!
//! - `{ army_rule = "max_points", points = N }`: the army costs at most `N` points, each unit
//!   priced by its printed `points` stat (0 when it prints none). Using this rule in packs that
//!   define no `points` stat is an error.
//! - `{ army_rule = "unique_once", rarity = R }`: a unit of rarity `R` (default `unique`) is
//!   taken at most once.
//! - `{ army_rule = "max_copies", copies = N }`: no unit is taken more than `N` times.
//! - `{ army_rule = "banned", units = { ... } }`: none of these units is taken.
//! - `{ army_rule = "require_faction", factions = { ... } }`: every unit belongs to one of them.
//!
//! `units` and `factions` take one name or a list.
//! - `{ army_rule = "max_footprint", cells = N }`: every figure together covers at most `N`
//!   spaces.
//!
//! A unit may be listed in several entries; copies count across all of them. Unit and faction
//! names are looked up as content lookup resolves them from the pack that defines the rule's
//! format or scenario, and compared by definition, so a bare name means that pack's unit first.
//! A name that resolves to nothing is an error, so a typo is never silently ignored.

use std::collections::BTreeMap;

use strata_kernel::{DefId, ModuleId, SlotId, Sym, Value, ValueError};
use strata_model::Descriptor;
use strata_rules::army::RULE_FIELD;
use strata_rules::{
    ArmyRules, ArmyUnit, ConfigSchema, LogicError, ModuleError, NameKind, Registrar, RuleModule,
};

use crate::shared::names;
use crate::toy::no_config;

/// The rarity `unique_once` limits unless the rule names another.
const UNIQUE: &str = "unique";

/// The army slot.
pub struct Army;

impl RuleModule for Army {
    fn id(&self) -> ModuleId {
        ModuleId::new("core:army")
    }

    fn slot(&self) -> Option<SlotId> {
        Some(SlotId::new("army"))
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        no_config()
    }

    fn register(&self, r: &mut Registrar, _: &Value) -> Result<(), ModuleError> {
        r.service::<dyn ArmyRules>(Box::new(Rules));
        Ok(())
    }
}

/// The native army rules.
struct Rules;

impl ArmyRules for Rules {
    fn check(
        &self,
        rule: &Descriptor,
        army: &[ArmyUnit],
        find: &dyn Fn(NameKind, &str) -> Option<DefId>,
    ) -> Result<Vec<String>, LogicError> {
        let rule = &rule.0;
        let name = rule.get(RULE_FIELD).and_then(Value::as_str);
        let name =
            name.map_err(|e| LogicError(format!("an army rule needs `{RULE_FIELD}`: {e}")))?;
        let bad = |e: ValueError| LogicError(format!("army_rule.{name}: {e}"));
        let limit = |key: &str| -> Result<i64, LogicError> {
            rule.get(key).and_then(Value::as_int).map_err(bad)
        };
        let resolve = |key: &str, kind: NameKind| -> Result<Vec<(Sym, DefId)>, LogicError> {
            let listed = names(rule, key).map_err(bad)?;
            let found = listed.into_iter().map(|n| match find(kind, n.as_str()) {
                Some(id) => Ok((n, id)),
                None => Err(LogicError(format!(
                    "army_rule.{name}: nothing is called `{n}`"
                ))),
            });
            found.collect()
        };
        Ok(match name {
            "max_points" => max_points(army, limit("points")?)?,
            "unique_once" => {
                let rarity = match rule.get("rarity") {
                    Ok(given) => given.as_str().map_err(bad)?,
                    Err(_) => UNIQUE,
                };
                unique_once(army, rarity)
            }
            "max_copies" => max_copies(army, limit("copies")?),
            "banned" => banned(army, &resolve("units", NameKind::Unit)?),
            "require_faction" => require_faction(army, &resolve("factions", NameKind::Faction)?),
            "max_footprint" => max_footprint(army, limit("cells")?),
            other => return Err(LogicError(format!("core:army has no rule `{other}`"))),
        })
    }
}

/// Copies of each unit across every entry that lists it, in name order.
fn copies(army: &[ArmyUnit]) -> BTreeMap<&Sym, (u32, &ArmyUnit)> {
    let mut found: BTreeMap<&Sym, (u32, &ArmyUnit)> = BTreeMap::new();
    for entry in army {
        found.entry(&entry.unit).or_insert((0, entry)).0 += u32::from(entry.copies);
    }
    found
}

fn max_points(army: &[ArmyUnit], limit: i64) -> Result<Vec<String>, LogicError> {
    let mut total = 0;
    for unit in army {
        let points = unit.points.ok_or_else(|| {
            LogicError("army_rule.max_points: the packs define no `points` stat".into())
        })?;
        total += points * i64::from(unit.copies);
    }
    if total <= limit {
        return Ok(Vec::new());
    }
    Ok(vec![format!(
        "the army costs {total} points, over the limit of {limit}"
    )])
}

fn unique_once(army: &[ArmyUnit], rarity: &str) -> Vec<String> {
    let all = copies(army).into_iter();
    let over = all.filter(|(_, (n, u))| u.rarity.as_str() == rarity && *n > 1);
    let say = |(unit, (n, _)): (&Sym, (u32, &ArmyUnit))| {
        format!("`{unit}` is {rarity} but is taken {n} times")
    };
    over.map(say).collect()
}

fn max_copies(army: &[ArmyUnit], limit: i64) -> Vec<String> {
    let all = copies(army).into_iter();
    let over = all.filter(|(_, (n, _))| i64::from(*n) > limit);
    let say = |(unit, (n, _)): (&Sym, (u32, &ArmyUnit))| {
        format!("`{unit}` is taken {n} times, over the limit of {limit}")
    };
    over.map(say).collect()
}

fn banned(army: &[ArmyUnit], bans: &[(Sym, DefId)]) -> Vec<String> {
    let all = copies(army).into_iter();
    let hit = all.filter(|(_, (_, u))| bans.iter().any(|(_, id)| *id == u.def));
    hit.map(|(unit, _)| format!("`{unit}` is banned")).collect()
}

fn require_faction(army: &[ArmyUnit], factions: &[(Sym, DefId)]) -> Vec<String> {
    let allowed = |u: &ArmyUnit| {
        let faction = u.faction_def;
        faction.is_some_and(|f| factions.iter().any(|(_, id)| *id == f))
    };
    let wanted: Vec<&str> = factions.iter().map(|(n, _)| n.as_str()).collect();
    let wanted = wanted.join(", ");
    let all = copies(army).into_iter();
    let outside = all.filter(|(_, (_, u))| !allowed(u));
    let say = |(unit, _): (&Sym, _)| format!("`{unit}` does not belong to {wanted}");
    outside.map(say).collect()
}

fn max_footprint(army: &[ArmyUnit], limit: i64) -> Vec<String> {
    let cells = army
        .iter()
        .map(|u| i64::from(u.footprint) * i64::from(u.copies));
    let total: i64 = cells.sum();
    if total <= limit {
        return Vec::new();
    }
    vec![format!(
        "the army covers {total} spaces, over the limit of {limit}"
    )]
}

#[cfg(test)]
mod tests;
