//! Maps and piece inventories (design §11.1).
//!
//! Pieces, features and the inventory refer to tiles and features by authored name, because maps
//! are plain data that is resolved against the loaded definitions when a match is built.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, Hex, Rotation, StableHash, Sym, Value};

use crate::vocab::{SourceLoc, Text};

/// A tile placed on the board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct PlacedPiece {
    /// Name of the tile.
    pub piece: Sym,
    /// Anchor cell.
    pub at: Hex,
    /// Rotation in 60° steps.
    pub rot: Rotation,
    /// Bottom level of the piece.
    pub level: u8,
}

/// A feature placed on the board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct PlacedFeature {
    /// Name of the feature.
    pub feature: Sym,
    /// Anchor cell.
    pub at: Hex,
    /// Rotation in 60° steps.
    pub rot: Rotation,
}

/// Build rules a map must satisfy.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct MapConstraints {
    /// Name of the inventory capping the pieces used.
    #[serde(default)]
    pub inventory: Option<Sym>,
    /// Whether every cell must rest on level 0 or on solid terrain.
    #[serde(default)]
    pub require_support: bool,
}

/// A board: placed pieces plus named zones and relic spots.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct MapDef {
    /// The map's handle.
    pub id: DefId,
    /// Display name.
    pub name: Text,
    /// Author names.
    #[serde(default)]
    pub authors: Vec<String>,
    /// Rendering orientation, `pointy` or `flat`.
    pub orientation: Sym,
    /// Tiles placed on the board.
    pub pieces: Vec<PlacedPiece>,
    /// Features placed on the board.
    #[serde(default)]
    pub features: Vec<PlacedFeature>,
    /// Named sets of columns that scenarios refer to.
    #[serde(default)]
    pub zones: BTreeMap<Sym, Vec<Hex>>,
    /// Named cells where relics may be placed.
    #[serde(default)]
    pub relic_spots: BTreeMap<Sym, Hex>,
    /// Build rules.
    #[serde(default)]
    pub constraints: MapConstraints,
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

/// A cap on the pieces a map may use.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct InventoryDef {
    /// The inventory's handle.
    pub id: DefId,
    /// Display name.
    #[serde(default)]
    pub name: Text,
    /// Available count per tile name.
    #[serde(default)]
    pub tiles: BTreeMap<Sym, u32>,
    /// Available count per feature name.
    #[serde(default)]
    pub features: BTreeMap<Sym, u32>,
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
