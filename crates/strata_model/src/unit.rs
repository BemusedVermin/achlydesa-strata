//! Units and figures (design §10.4).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, StableHash, Sym, Value};

use crate::ability::AbilityDef;
use crate::vocab::{SourceLoc, Text};

/// How a figure is seen for line of sight, in sub-level units (12 per level).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct LosProfile {
    /// Height of the point other figures aim at.
    pub target: u16,
    /// `(bottom, top)` of the volume that can be hit.
    pub hit_zone: (u16, u16),
    /// Names of the sample points, such as `center` and `corners`.
    pub samples: Vec<Sym>,
}

/// One kind of figure within a unit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct FigureDef {
    /// Name of the figure within its unit.
    pub id: Sym,
    /// How many of this figure the unit fields.
    pub count: u8,
    /// The base shape.
    pub footprint: DefId,
    /// Height in levels.
    pub height: u8,
    /// The profile line of sight uses.
    pub los: LosProfile,
    /// Name of the sprite set.
    #[serde(default)]
    pub visual: Option<Sym>,
}

/// Printed card information.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct CardInfo {
    /// Asset name of the card art.
    #[serde(default)]
    pub art: Option<Sym>,
    /// Flavor text.
    #[serde(default)]
    pub flavor: Option<Text>,
}

/// A selectable unit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct UnitDef {
    /// The unit's handle.
    pub id: DefId,
    /// Display name.
    pub name: Text,
    /// Vocabulary owned by the units module, such as `squad`.
    pub kind: Sym,
    /// Vocabulary owned by the army module, such as `common`.
    pub rarity: Sym,
    /// The unit's allegiance.
    #[serde(default)]
    pub faction: Option<DefId>,
    /// Trait values keyed by trait key.
    #[serde(default)]
    pub traits: BTreeMap<Sym, Sym>,
    /// Base stat values keyed by stat.
    pub stats: BTreeMap<DefId, i32>,
    /// The unit's figures.
    pub figures: Vec<FigureDef>,
    /// The unit's abilities.
    #[serde(default)]
    pub abilities: Vec<AbilityDef>,
    /// Printed card information.
    #[serde(default)]
    pub card: CardInfo,
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
