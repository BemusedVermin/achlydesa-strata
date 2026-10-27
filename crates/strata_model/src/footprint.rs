//! Footprints: base shapes and move modes (design §10.4).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, Shape, StableHash, Sym, Value};

use crate::vocab::{SourceLoc, Text};

/// How a multi-cell footprint moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(rename_all = "lowercase")]
pub enum MoveMode {
    /// The shape translates as a whole.
    Rigid,
    /// The rear follows the leader's path.
    Snake,
}

/// A base shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct FootprintDef {
    /// The footprint's handle.
    pub id: DefId,
    /// Display name.
    #[serde(default)]
    pub name: Text,
    /// Axial offsets from the anchor cell.
    pub cells: Shape,
    /// How the footprint moves.
    #[serde(rename = "move")]
    pub move_mode: MoveMode,
    /// Largest allowed level spread across the cells.
    pub max_spread: u8,
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
