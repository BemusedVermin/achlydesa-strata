//! The army legality service interface (design §11.3, §11.4).
//!
//! A module in the `army` slot provides [`ArmyRules`], which checks an army against one native
//! army rule, such as `{ army_rule = "max_points", points = 500 }`. [`validate`] applies every
//! rule of a format and a scenario, both sets at once, and returns each violation with the rule
//! that produced it. A rule written as a Lua function is called through a [`ScriptCaller`] with
//! the army as its subject. Names a rule mentions, such as banned units, are looked up through
//! [`ArmyNames`] from the pack that defines the rule's format or scenario.

use std::collections::BTreeMap;

use strata_kernel::{DefId, Sym, Value};
use strata_model::{Descriptor, FormatDef, Logic, ScenarioDef};

use crate::native::logic_name;
use crate::query::{EvalCtx, LogicError, ScriptCaller};

/// The descriptor field that names a native army rule.
pub const RULE_FIELD: &str = "army_rule";
/// The message for a Lua army rule that returns `false`.
pub const SCRIPT_FAILED: &str = "the army does not meet this rule";

/// One entry of an army list, with what the rules need to know about its unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArmyUnit {
    /// The unit's full name, such as `strata_std:ironclad_line`.
    pub unit: Sym,
    /// The unit's definition.
    pub def: DefId,
    /// How many copies the entry takes.
    pub copies: u8,
    /// The printed `points` of one copy, 0 when the unit prints none. `None` when the packs
    /// define no `points` stat, so no rule can price the army.
    pub points: Option<i64>,
    /// The unit's rarity, such as `common` or `unique`.
    pub rarity: Sym,
    /// The full name of the unit's faction, if it has one.
    pub faction: Option<Sym>,
    /// The definition of the unit's faction, if it has one.
    pub faction_def: Option<DefId>,
    /// How many spaces the footprints of all figures of one copy cover.
    pub footprint: u32,
}

/// Which definition an army rule came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuleOrigin {
    /// The format's `army` list.
    Format,
    /// The scenario's `army` list.
    Scenario,
}

/// One way an army breaks one rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArmyViolation {
    /// Where the rule is listed.
    pub origin: RuleOrigin,
    /// The rule's position in its list.
    pub index: usize,
    /// The rule's name, such as `max_points`, or `script` for a Lua function.
    pub rule: Sym,
    /// What is wrong, for the army builder to show.
    pub message: String,
}

/// What kind of definition an army rule names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameKind {
    /// A unit, as `banned` lists them.
    Unit,
    /// A faction, as `require_faction` lists them.
    Faction,
}

/// Looks up the names army rules mention.
pub trait ArmyNames {
    /// The definition a rule listed in `origin` means by `name`, looked up the way content
    /// lookup resolves an authored name in the pack that defines that format or scenario.
    /// `None` when it names nothing.
    fn find(&self, origin: RuleOrigin, kind: NameKind, name: &str) -> Option<DefId>;
}

/// Army legality, as the module in the `army` slot defines it.
pub trait ArmyRules: Send + Sync {
    /// Checks `army` against one native rule and describes every way it breaks it; an empty
    /// list means the army keeps the rule. `names` resolves the names the rule lists. A rule
    /// the module does not know, or one that names a definition `names` cannot find, is an
    /// error.
    fn check(
        &self,
        rule: &Descriptor,
        army: &[ArmyUnit],
        names: &dyn Fn(NameKind, &str) -> Option<DefId>,
    ) -> Result<Vec<String>, LogicError>;
}

/// The army as a script sees it: a list of `{ unit, copies, points, rarity, faction, footprint }`,
/// without `points` when the packs define no `points` stat.
pub fn army_value(army: &[ArmyUnit]) -> Value {
    let entry = |u: &ArmyUnit| {
        let mut map = BTreeMap::from([
            (Sym::new("unit"), Value::Str(u.unit.clone())),
            (Sym::new("copies"), Value::Int(i64::from(u.copies))),
            (Sym::new("rarity"), Value::Str(u.rarity.clone())),
            (Sym::new("footprint"), Value::Int(i64::from(u.footprint))),
        ]);
        if let Some(points) = u.points {
            map.insert(Sym::new("points"), Value::Int(points));
        }
        if let Some(faction) = &u.faction {
            map.insert(Sym::new("faction"), Value::Str(faction.clone()));
        }
        Value::Map(map)
    };
    Value::List(army.iter().map(entry).collect())
}

/// What a script rule returned, as violation messages: nil, `true` or an empty table when the
/// army passes; `false` for one violation with a generic message; a string for one violation
/// with that message; a list of strings for several.
fn script_messages(found: Value) -> Result<Vec<String>, LogicError> {
    let bad = |what: &str| LogicError(format!("an army rule script returned {what}"));
    match found {
        Value::Nil | Value::Bool(true) => Ok(Vec::new()),
        Value::Bool(false) => Ok(vec![SCRIPT_FAILED.to_owned()]),
        Value::Str(message) => Ok(vec![message.as_str().to_owned()]),
        Value::List(items) => items
            .iter()
            .map(|i| {
                i.as_str()
                    .map(str::to_owned)
                    .map_err(|_| bad("a list holding a non-string"))
            })
            .collect(),
        // An empty Lua table freezes as an empty map.
        Value::Map(map) if map.is_empty() => Ok(Vec::new()),
        other => Err(bad(other.type_name())),
    }
}

/// Checks `army` against every rule of `format` and of `scenario`: the format's first, then the
/// scenario's, each in list order. A scenario can only add rules, so both sets always apply
/// (design §11.3). Fails if a rule is malformed, names a definition `names` cannot find, or its
/// script fails.
pub fn validate(
    rules: &dyn ArmyRules,
    scripts: &dyn ScriptCaller,
    names: &dyn ArmyNames,
    army: &[ArmyUnit],
    format: Option<&FormatDef>,
    scenario: Option<&ScenarioDef>,
) -> Result<Vec<ArmyViolation>, LogicError> {
    let format = format.map(|f| (RuleOrigin::Format, &f.army));
    let scenario = scenario.map(|s| (RuleOrigin::Scenario, &s.army));
    let subject = army_value(army);
    let source = Sym::new("army");
    let mut found = Vec::new();
    for (origin, list) in format.into_iter().chain(scenario) {
        let find = |kind, name: &str| names.find(origin, kind, name);
        for (index, rule) in list.iter().enumerate() {
            let messages = match rule {
                Logic::Native(descriptor) => rules.check(descriptor, army, &find)?,
                Logic::Script(script) => {
                    let cx = EvalCtx {
                        source: &source,
                        subject: &subject,
                        context: &Value::Nil,
                    };
                    script_messages(scripts.call(script, &cx)?)?
                }
            };
            let name = logic_name(rule, RULE_FIELD);
            found.extend(messages.into_iter().map(|message| ArmyViolation {
                origin,
                index,
                rule: name.clone(),
                message,
            }));
        }
    }
    Ok(found)
}
