//! Footprints: base shapes and move modes (design §10.4).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, Hex, Rotation, Shape, Space, StableHash, Sym, Value};
use thiserror::Error;

use crate::board::CompiledBoard;
use crate::vocab::{SourceLoc, Text};

#[cfg(test)]
mod tests;

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
#[serde(deny_unknown_fields)]
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

/// A footprint that cannot stand where it was put (design §7.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum FootprintError {
    /// The anchor space is not a surface of the board.
    #[error("{0:?} is not a surface")]
    NotASurface(Space),
    /// A cell of the footprint has no surface to stand on.
    #[error("cell {0:?} has no surface")]
    NoSurface(Hex),
    /// The cells' levels differ by more than the footprint allows.
    #[error("the cells span {spread} levels, more than the {max} allowed")]
    Spread {
        /// The difference between the highest and lowest cell.
        spread: u32,
        /// The footprint's `max_spread`.
        max: u8,
    },
}

impl FootprintDef {
    /// The spaces the footprint covers with its anchor on `anchor`, turned by `rotation`.
    ///
    /// The anchor must be a surface. Every other cell stands on its surface nearest the anchor's
    /// level (the lower one on a tie), and the cells' levels may differ by at most `max_spread`.
    pub fn spaces(
        &self,
        board: &CompiledBoard,
        anchor: Space,
        rotation: Rotation,
    ) -> Result<Vec<Space>, FootprintError> {
        if !board.surfaces(anchor.hex()).iter().any(|s| s.z == anchor.z) {
            return Err(FootprintError::NotASurface(anchor));
        }
        let mut spaces = Vec::with_capacity(self.cells.len());
        for cell in self
            .cells
            .rotated(rotation)
            .translated(anchor.hex())
            .cells()
        {
            let z = if *cell == anchor.hex() {
                Some(anchor.z)
            } else {
                let levels = board.surfaces(*cell).iter().map(|s| s.z);
                levels.min_by_key(|z| (z.abs_diff(anchor.z), *z))
            };
            let z = z.ok_or(FootprintError::NoSurface(*cell))?;
            spaces.push(Space::at(*cell, z));
        }
        let levels = spaces.iter().map(|s| s.z);
        let spread = levels.clone().max().unwrap_or(0) - levels.min().unwrap_or(0);
        if spread > u32::from(self.max_spread) {
            return Err(FootprintError::Spread {
                spread,
                max: self.max_spread,
            });
        }
        Ok(spaces)
    }
}
