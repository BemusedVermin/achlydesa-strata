//! Terrain, tiles and features (design §10.3).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, Hex, Shape, StableHash, Sym, Value};

use crate::vocab::{SourceLoc, Text};

/// An opaque reference to an ability definition, until abilities are defined.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(transparent)]
pub struct AbilityRef(pub DefId);

/// Surface behavior of a tile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct TerrainDef {
    /// The terrain's handle.
    pub id: DefId,
    /// Display name.
    pub name: Text,
    /// Whether a figure can stand on it.
    pub standable: bool,
    /// Abilities attached to the terrain.
    #[serde(default)]
    pub abilities: Vec<AbilityRef>,
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

/// A physical map piece: a shape of one terrain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct TileDef {
    /// The tile's handle.
    pub id: DefId,
    /// Display name.
    pub name: Text,
    /// The terrain covering every cell.
    pub terrain: DefId,
    /// Thickness in levels.
    pub thickness: u8,
    /// Axial offsets of the cells.
    pub shape: Shape,
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

/// A vertical span of a feature cell, in levels above the surface it is placed on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct Segment {
    /// Bottom of the span, in levels.
    pub from: u8,
    /// Top of the span, in levels.
    pub to: u8,
    /// Whether a figure can stand on top of the span.
    #[serde(default)]
    pub top: bool,
}

/// One cell of a feature and its solid spans.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct FeatureCell {
    /// Axial offset from the feature's anchor.
    pub at: Hex,
    /// Solid spans in this cell.
    pub segments: Vec<Segment>,
}

/// A non-terrain board object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct FeatureDef {
    /// The feature's handle.
    pub id: DefId,
    /// Display name.
    pub name: Text,
    /// Whether the feature blocks line of sight.
    pub blocks_los: bool,
    /// The cells it occupies.
    pub cells: Vec<FeatureCell>,
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
