//! Rulesets and Lua rule modules (design §6.2, §6.3).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, ModuleId, SlotId, StableHash, Sym, Value};

use crate::ability::ScriptRef;
use crate::vocab::{SourceLoc, Text};

/// A module and the configuration it is registered with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct ModuleRef {
    /// The module, such as `core:combat`.
    pub id: ModuleId,
    /// Configuration values keyed by option.
    #[serde(default)]
    pub config: BTreeMap<Sym, Value>,
}

/// What a ruleset allows undoing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(rename_all = "snake_case")]
pub enum UndoPolicy {
    /// Takebacks until a die is rolled or hidden information is revealed.
    #[default]
    UntilRandomOrReveal,
}

/// A named composition of rule modules.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct RulesetDef {
    /// The ruleset's handle.
    pub id: DefId,
    /// Display name.
    #[serde(default)]
    pub name: Text,
    /// The ruleset this one builds on, possibly qualified by pack.
    #[serde(default)]
    pub extends: Option<Sym>,
    /// Names of the stats in play.
    #[serde(default)]
    pub stats: Vec<Sym>,
    /// Names of the dice in play.
    #[serde(default)]
    pub dice: Vec<Sym>,
    /// Module per slot; `None` empties the slot.
    #[serde(default)]
    pub slots: BTreeMap<SlotId, Option<ModuleRef>>,
    /// Add-on modules, which fill no slot.
    #[serde(default)]
    pub addons: Vec<ModuleRef>,
    /// Whether abilities are in play.
    #[serde(default = "enabled")]
    pub abilities: bool,
    /// What may be undone.
    #[serde(default)]
    pub undo: UndoPolicy,
    /// Free-form tags.
    #[serde(default)]
    pub tags: BTreeSet<Sym>,
    /// Data the engine ignores and tools may use.
    #[serde(default)]
    pub meta: BTreeMap<Sym, Value>,
    /// Where the definition was authored.
    #[serde(default)]
    pub source: SourceLoc,
}

fn enabled() -> bool {
    true
}

/// One option in a Lua module's configuration schema.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct ConfigField {
    /// The value type, such as `int`.
    #[serde(rename = "type")]
    pub kind: Sym,
    /// The value used when the ruleset sets none.
    #[serde(default)]
    pub default: Value,
}

/// A rule module written in Lua.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct RuleModuleDef {
    /// The module's handle.
    pub id: DefId,
    /// Display name.
    #[serde(default)]
    pub name: Text,
    /// The slot it fills; `None` for an add-on.
    #[serde(default)]
    pub slot: Option<SlotId>,
    /// Modules it requires.
    #[serde(default)]
    pub depends: Vec<ModuleId>,
    /// Its configuration schema.
    #[serde(default)]
    pub config: BTreeMap<Sym, ConfigField>,
    /// The registration function.
    pub register: ScriptRef,
    /// Free-form tags.
    #[serde(default)]
    pub tags: BTreeSet<Sym>,
    /// Data the engine ignores and tools may use.
    #[serde(default)]
    pub meta: BTreeMap<Sym, Value>,
    /// Where the definition was authored.
    #[serde(default)]
    pub source: SourceLoc,
}
