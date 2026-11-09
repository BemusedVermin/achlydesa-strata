//! `core:army`: army legality, from points, uniqueness, copies, bans, factions and footprint
//! (design §6.1, §11.3, §11.4).
//!
//! The module provides [`ArmyRules`] for the native rules the `army_rule.*` constructors write:
//!
//! - `{ army_rule = "max_points", points = N }`: the army costs at most `N` points.
//! - `{ army_rule = "unique_once", rarity = R }`: a unit of rarity `R` (default `unique`) is
//!   taken at most once.
//! - `{ army_rule = "max_copies", copies = N }`: no unit is taken more than `N` times.
//! - `{ army_rule = "banned", units = { ... } }`: none of these units is taken.
//! - `{ army_rule = "require_faction", factions = { ... } }`: every unit belongs to one of them.
//! - `{ army_rule = "max_footprint", cells = N }`: every figure together covers at most `N`
//!   spaces.
//!
//! A unit may be listed in several entries; copies count across all of them. Unit and faction
//! names match by full name (`pack:name`) or bare name.

use std::collections::BTreeMap;

use strata_kernel::{ModuleId, SlotId, Sym, Value, ValueError};
use strata_model::Descriptor;
use strata_rules::army::RULE_FIELD;
use strata_rules::{
    ArmyRules, ArmyUnit, ConfigSchema, LogicError, ModuleError, Registrar, RuleModule,
};

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
    fn check(&self, rule: &Descriptor, army: &[ArmyUnit]) -> Result<Vec<String>, LogicError> {
        let rule = &rule.0;
        let name = rule.get(RULE_FIELD).and_then(Value::as_str);
        let name =
            name.map_err(|e| LogicError(format!("an army rule needs `{RULE_FIELD}`: {e}")))?;
        let bad = |e: ValueError| LogicError(format!("army_rule.{name}: {e}"));
        let limit = |key: &str| -> Result<i64, LogicError> {
            rule.get(key).and_then(Value::as_int).map_err(bad)
        };
        Ok(match name {
            "max_points" => max_points(army, limit("points")?),
            "unique_once" => {
                let rarity = match rule.get("rarity") {
                    Ok(given) => given.as_str().map_err(bad)?,
                    Err(_) => UNIQUE,
                };
                unique_once(army, rarity)
            }
            "max_copies" => max_copies(army, limit("copies")?),
            "banned" => banned(army, &names(rule, "units").map_err(bad)?),
            "require_faction" => require_faction(army, &names(rule, "factions").map_err(bad)?),
            "max_footprint" => max_footprint(army, limit("cells")?),
            other => return Err(LogicError(format!("core:army has no rule `{other}`"))),
        })
    }
}

/// The strings listed under `key`; a missing key or an empty table lists none.
fn names(rule: &Value, key: &str) -> Result<Vec<Sym>, ValueError> {
    match rule.get(key) {
        Err(_) | Ok(Value::Nil) => Ok(Vec::new()),
        Ok(Value::Map(map)) if map.is_empty() => Ok(Vec::new()),
        Ok(list) => {
            let items = list.as_list()?.iter();
            items.map(|v| v.as_str().map(Sym::new)).collect()
        }
    }
}

/// Whether `full` (such as `pack:name`) is `wanted`, written in full or bare.
fn same_name(full: &str, wanted: &str) -> bool {
    full == wanted || full.split_once(':').is_some_and(|(_, bare)| bare == wanted)
}

/// Copies of each unit across every entry that lists it, in name order.
fn copies(army: &[ArmyUnit]) -> BTreeMap<&Sym, (u32, &ArmyUnit)> {
    let mut found: BTreeMap<&Sym, (u32, &ArmyUnit)> = BTreeMap::new();
    for entry in army {
        found.entry(&entry.unit).or_insert((0, entry)).0 += u32::from(entry.copies);
    }
    found
}

fn max_points(army: &[ArmyUnit], limit: i64) -> Vec<String> {
    let total: i64 = army.iter().map(|u| u.points * i64::from(u.copies)).sum();
    if total <= limit {
        return Vec::new();
    }
    vec![format!(
        "the army costs {total} points, over the limit of {limit}"
    )]
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

fn banned(army: &[ArmyUnit], bans: &[Sym]) -> Vec<String> {
    let all = copies(army).into_keys();
    let hit = all.filter(|unit| bans.iter().any(|b| same_name(unit.as_str(), b.as_str())));
    hit.map(|unit| format!("`{unit}` is banned")).collect()
}

fn require_faction(army: &[ArmyUnit], factions: &[Sym]) -> Vec<String> {
    let allowed = |u: &ArmyUnit| {
        let faction = u.faction.as_ref();
        faction.is_some_and(|f| factions.iter().any(|w| same_name(f.as_str(), w.as_str())))
    };
    let wanted: Vec<&str> = factions.iter().map(Sym::as_str).collect();
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
