//! Vocabulary definitions: stats, dice, markers, traits and factions (design §10.2).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, StableHash, Sym, Value};

/// Display text for names and descriptions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(transparent)]
pub struct Text(pub String);

/// The file and line a definition was authored at, for every later error.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct SourceLoc {
    /// Path of the source file.
    pub file: String,
    /// One-based line number.
    pub line: u32,
}

/// A stat with inclusive bounds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct StatDef {
    /// The stat's handle.
    pub id: DefId,
    /// Display name.
    pub name: Text,
    /// Smallest allowed value.
    pub min: i32,
    /// Largest allowed value.
    pub max: i32,
    /// Short label for cards and tables.
    #[serde(default)]
    pub abbrev: Option<Text>,
    /// Sort position when stats are listed.
    #[serde(default)]
    pub order: i32,
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

/// What a die rolls: named faces, or an inclusive numeric range.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(rename_all = "lowercase")]
pub enum DieKind {
    /// Symbol faces; repeated symbols weight the roll.
    Faces(Vec<Sym>),
    /// Inclusive `(low, high)` integer range.
    Range(i32, i32),
}

/// A die.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct DieDef {
    /// The die's handle.
    pub id: DefId,
    /// Display name.
    pub name: Text,
    /// Faces or numeric range.
    #[serde(flatten)]
    pub kind: DieKind,
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

/// A marker such as a wound or a status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct MarkerDef {
    /// The marker's handle.
    pub id: DefId,
    /// Display name.
    pub name: Text,
    /// How repeated markers combine, such as `count`.
    pub stacking: Sym,
    /// Who can see the marker, such as `public`.
    pub visibility: Sym,
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

/// The values a trait key accepts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(rename_all = "lowercase")]
pub enum TraitKeyDef {
    /// Any symbol.
    Open,
    /// Only the listed symbols.
    Allowed(Vec<Sym>),
}

/// The set of trait keys units may carry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct TraitsDef {
    /// The definition's handle.
    pub id: DefId,
    /// Display name.
    pub name: Text,
    /// Allowed values per trait key.
    pub keys: BTreeMap<Sym, TraitKeyDef>,
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

/// An allegiance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct FactionDef {
    /// The faction's handle.
    pub id: DefId,
    /// Display name.
    pub name: Text,
    /// Hex color such as `#B5893A`.
    pub color: String,
    /// Asset name of the emblem.
    #[serde(default)]
    pub emblem: Option<Sym>,
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
