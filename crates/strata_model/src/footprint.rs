//! Footprints: base shapes and move modes (design §10.4).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, Hex, Rotation, Shape, Space, StableHash, Sym, Value};

use crate::board::{CompiledBoard, Surface};
use crate::vocab::{SourceLoc, Text};

mod check;
#[cfg(test)]
mod tests;

pub use self::check::{FootprintError, MAX_SNAKE_CELLS};

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

impl FootprintDef {
    /// The cells of the footprint at `anchor` turned by `rotation`, in offset order.
    pub fn cells_at(&self, anchor: Hex, rotation: Rotation) -> Vec<Hex> {
        let turned = self.cells.cells().iter().map(|c| c.rotate(rotation));
        turned.map(|c| c.add(anchor)).collect()
    }

    /// The spaces the footprint covers with its anchor on `anchor`, in offset order, or `None`
    /// when the anchor is not a surface, a cell has no surface, or the cells' levels spread
    /// wider than `max_spread`.
    ///
    /// Each cell stands on the surface of its column nearest the anchor's level, the lower one
    /// on a tie, so a figure on a bridge deck stays on the deck.
    pub fn spaces(
        &self,
        board: &CompiledBoard,
        anchor: Space,
        rotation: Rotation,
    ) -> Option<Vec<(Space, Surface)>> {
        self.spaces_where(board, anchor, rotation, |_, _| true)
    }

    /// [`FootprintDef::spaces`], with every cell standing only where `fits` allows, such as on
    /// passable terrain with headroom.
    ///
    /// Each cell tries the surfaces of its column within `max_spread` of the anchor's level,
    /// nearest first and the lower one on a tie, and stands on the first that fits. The anchor
    /// cell only ever stands on `anchor`. Each cell chooses on its own, so a cell that could only
    /// fit by moving another cell is not found.
    pub fn spaces_where(
        &self,
        board: &CompiledBoard,
        anchor: Space,
        rotation: Rotation,
        mut fits: impl FnMut(Space, &Surface) -> bool,
    ) -> Option<Vec<(Space, Surface)>> {
        if !board.surfaces(anchor.hex()).iter().any(|s| s.z == anchor.z) {
            return None;
        }
        let spread = u32::from(self.max_spread);
        let mut placed = Vec::with_capacity(self.cells.len());
        for hex in self.cells_at(anchor.hex(), rotation) {
            let in_reach = |s: &&Surface| {
                if hex == anchor.hex() {
                    s.z == anchor.z
                } else {
                    s.z.abs_diff(anchor.z) <= spread
                }
            };
            let mut near: Vec<&Surface> = board.surfaces(hex).iter().filter(in_reach).collect();
            near.sort_by_key(|s| (s.z.abs_diff(anchor.z), s.z));
            let found = near.into_iter().find(|s| fits(Space::at(hex, s.z), s))?;
            placed.push((Space::at(hex, found.z), *found));
        }
        let levels = placed.iter().map(|(s, _)| s.z);
        let (low, high) = (levels.clone().min()?, levels.max()?);
        (high - low <= spread).then_some(placed)
    }
}
