//! Relics (design §10.6).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, StableHash, Sym, Value};

use crate::ability::{AbilityDef, Logic};
use crate::vocab::{SourceLoc, Text};

/// When a relic takes effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(rename_all = "snake_case")]
pub enum RelicMode {
    /// Grants abilities while someone occupies it.
    WhileOccupied,
    /// Runs its effect once when claimed.
    OnClaim,
}

/// Abilities a relic gives its holder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct RelicGrant {
    /// Who receives the abilities, such as `occupant_army`.
    pub to: Sym,
    /// The abilities granted.
    pub abilities: Vec<AbilityDef>,
}

/// A claimable board object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct RelicDef {
    /// The relic's handle.
    pub id: DefId,
    /// Display name.
    pub name: Text,
    /// When it takes effect.
    pub mode: RelicMode,
    /// How it is claimed, such as `ends_move_on`.
    pub claim: Sym,
    /// Whether it may be placed face down.
    #[serde(default)]
    pub face_down_allowed: bool,
    /// Abilities granted; absent for an objective.
    #[serde(default)]
    pub grants: Option<RelicGrant>,
    /// The one-shot effect of an `on_claim` relic.
    #[serde(default)]
    pub effect: Option<Logic>,
    /// Name of the sprite set.
    #[serde(default)]
    pub visual: Option<Sym>,
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
