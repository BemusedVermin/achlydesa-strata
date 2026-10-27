//! Abilities and the logic they are built from (design §10.5).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, PackId, StableHash, Sym, Value, WindowId};

use crate::vocab::Text;

/// A tagged tree of conditions, selectors, values or effects that Rust evaluates natively.
///
/// It stays a [`Value`] until the descriptor vocabulary is typed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(transparent)]
pub struct Descriptor(pub Value);

/// A handle to a Lua function, so the registry holds plain data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct ScriptRef {
    /// The pack that defines the function.
    pub pack: PackId,
    /// The definition that contains the function.
    pub def: DefId,
    /// Where the function sits inside the definition, such as `after.figure_destroyed`.
    pub path: Sym,
}

/// A condition, selector, value or effect: the native fast path or a Lua slow path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(rename_all = "lowercase")]
pub enum Logic {
    /// Evaluated in Rust and readable by the AI.
    Native(Descriptor),
    /// A Lua function.
    Script(ScriptRef),
}

/// Whom an ability belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// One figure.
    Figure,
    /// A whole unit.
    Unit,
    /// A whole army.
    Army,
    /// Whoever holds the relic.
    Holder,
    /// Whoever stands on the terrain.
    Occupant,
}

/// The archetype an ability was built from, for the AI and coverage stats.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct ArchetypeInfo {
    /// The family, such as `stat_bonus`.
    pub family: Sym,
    /// The parameters the constructor was called with.
    #[serde(default)]
    pub params: BTreeMap<Sym, Value>,
}

/// The step of a query a modifier applies in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(rename_all = "lowercase")]
pub enum ModifierLayer {
    /// Replaces the base value.
    Set,
    /// Adds to the value.
    Add,
    /// Multiplies the value.
    Mul,
    /// Applies after every other layer.
    Final,
}

/// Changes the result of a query.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct ModifierDef {
    /// The query modified, such as `defense` or `can_enter`.
    pub query: Sym,
    /// The layer it applies in.
    pub layer: ModifierLayer,
    /// Whom it applies to.
    #[serde(default)]
    pub select: Option<Logic>,
    /// When it applies.
    #[serde(default)]
    pub when: Option<Logic>,
    /// The amount or vote.
    pub value: Logic,
    /// Whether a boolean vote beats `deny`.
    #[serde(default, rename = "override")]
    pub overrides: bool,
}

/// Something a figure can do with activation slots.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct ActionDef {
    /// The action's name.
    pub id: Sym,
    /// Slots the action consumes.
    pub slots: Vec<Sym>,
    /// Which targets are legal.
    #[serde(default)]
    pub targets: Option<Logic>,
    /// What happens when it is taken.
    pub resolve: Logic,
}

/// How often a trigger may fire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(rename_all = "snake_case")]
pub enum TriggerLimit {
    /// Once per turn.
    OncePerTurn,
    /// Once per round.
    OncePerRound,
    /// Once per game.
    OncePerGame,
}

/// Reacts after an event applies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct TriggerDef {
    /// The event kind watched.
    pub event: Sym,
    /// Which events count.
    #[serde(default)]
    pub filter: Option<Logic>,
    /// How often it may fire.
    #[serde(default)]
    pub limit: Option<TriggerLimit>,
    /// What happens.
    pub effect: Logic,
}

/// Runs before an event applies and may change or cancel it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct ReplacementDef {
    /// The event kind intercepted.
    pub event: Sym,
    /// Which events count.
    #[serde(default)]
    pub filter: Option<Logic>,
    /// What replaces the event.
    pub replace: Logic,
}

/// A response offered inside a named window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct ReactionDef {
    /// The window it is offered in.
    pub window: WindowId,
    /// When it is eligible.
    #[serde(default)]
    pub when: Option<Logic>,
    /// What happens.
    pub effect: Logic,
}

/// A behavior attached to a figure, unit, army, relic or terrain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct AbilityDef {
    /// Display name.
    pub name: Text,
    /// Card text, generated or overridden.
    pub text: Text,
    /// Whom the ability belongs to.
    pub scope: Scope,
    /// The archetype it was built from, if any.
    #[serde(default)]
    pub archetype: Option<ArchetypeInfo>,
    /// Query modifiers.
    #[serde(default)]
    pub modifiers: Vec<ModifierDef>,
    /// Actions it grants.
    #[serde(default)]
    pub actions: Vec<ActionDef>,
    /// Event replacements.
    #[serde(default)]
    pub replacements: Vec<ReplacementDef>,
    /// Event triggers.
    #[serde(default)]
    pub triggers: Vec<TriggerDef>,
    /// Window reactions.
    #[serde(default)]
    pub reactions: Vec<ReactionDef>,
    /// Per-instance state with its defaults.
    #[serde(default)]
    pub vars: BTreeMap<Sym, Value>,
}
