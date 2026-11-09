//! The army legality service interface (design §11.3, §11.4).
//!
//! A module in the `army` slot provides [`ArmyRules`], which checks an army against one native
//! army rule, such as `{ army_rule = "max_points", points = 500 }`. [`validate`] applies every
//! rule of a format and a scenario, both sets at once, and returns each violation with the rule
//! that produced it. A rule written as a Lua function is called through a [`ScriptCaller`] with
//! the army as its subject.

use std::collections::BTreeMap;

use strata_kernel::{Sym, Value};
use strata_model::{Descriptor, FormatDef, Logic, ScenarioDef};

use crate::query::{EvalCtx, LogicError, ScriptCaller};

/// The descriptor field that names a native army rule.
pub const RULE_FIELD: &str = "army_rule";
/// The rule name reported for an army rule written as a Lua function.
pub const SCRIPT_RULE: &str = "script";

/// One entry of an army list, with what the rules need to know about its unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArmyUnit {
    /// The unit's full name, such as `strata_std:ironclad_line`.
    pub unit: Sym,
    /// How many copies the entry takes.
    pub copies: u8,
    /// The printed `points` of one copy.
    pub points: i64,
    /// The unit's rarity, such as `common` or `unique`.
    pub rarity: Sym,
    /// The full name of the unit's faction, if it has one.
    pub faction: Option<Sym>,
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

/// Army legality, as the module in the `army` slot defines it.
pub trait ArmyRules: Send + Sync {
    /// Checks `army` against one native rule and describes every way it breaks it; an empty
    /// list means the army keeps the rule. A rule the module does not know is an error.
    fn check(&self, rule: &Descriptor, army: &[ArmyUnit]) -> Result<Vec<String>, LogicError>;
}

/// The name of an army rule: the descriptor's `army_rule` field, or `script`.
pub fn rule_name(rule: &Logic) -> Sym {
    match rule {
        Logic::Native(Descriptor(value)) => {
            let name = value.get(RULE_FIELD).and_then(Value::as_str);
            Sym::new(name.unwrap_or("unnamed"))
        }
        Logic::Script(_) => Sym::new(SCRIPT_RULE),
    }
}

/// The army as a script sees it: a list of `{ unit, copies, points, rarity, faction, footprint }`.
pub fn army_value(army: &[ArmyUnit]) -> Value {
    let entry = |u: &ArmyUnit| {
        let mut map = BTreeMap::from([
            (Sym::new("unit"), Value::Str(u.unit.clone())),
            (Sym::new("copies"), Value::Int(i64::from(u.copies))),
            (Sym::new("points"), Value::Int(u.points)),
            (Sym::new("rarity"), Value::Str(u.rarity.clone())),
            (Sym::new("footprint"), Value::Int(i64::from(u.footprint))),
        ]);
        if let Some(faction) = &u.faction {
            map.insert(Sym::new("faction"), Value::Str(faction.clone()));
        }
        Value::Map(map)
    };
    Value::List(army.iter().map(entry).collect())
}

/// What a script rule returned, as violation messages: nil or `true` for none, a string for
/// one, a list of strings for several.
fn script_messages(found: Value) -> Result<Vec<String>, LogicError> {
    let bad = |what: &str| LogicError(format!("an army rule script returned {what}"));
    match found {
        Value::Nil | Value::Bool(true) => Ok(Vec::new()),
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
/// (design §11.3). Fails if a rule is malformed or its script fails.
pub fn validate(
    rules: &dyn ArmyRules,
    scripts: &dyn ScriptCaller,
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
        for (index, rule) in list.iter().enumerate() {
            let messages = match rule {
                Logic::Native(descriptor) => rules.check(descriptor, army)?,
                Logic::Script(script) => {
                    let cx = EvalCtx {
                        source: &source,
                        subject: &subject,
                        context: &Value::Nil,
                    };
                    script_messages(scripts.call(script, &cx)?)?
                }
            };
            let name = rule_name(rule);
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
