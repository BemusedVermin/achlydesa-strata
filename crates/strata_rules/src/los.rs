//! The line-of-sight service interface (design §7.5, §7.6).
//!
//! A module in the `los` slot provides [`LineOfSight`]. It asks a [`LosWorld`], which the host
//! implements over the board and the loaded content, for what stands where.

use strata_kernel::{Blocker, Frac, Hex, Seg3, Space};
use strata_model::FigureId;

use crate::query::LogicError;

/// A figure's camera-independent volume, in sub-level heights measured from the board floor.
///
/// Each cell stands on its own surface (design §7.3), so each carries its own prism (§7.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FigureVolume {
    /// The anchor cell.
    pub anchor: Hex,
    /// Height of the point other figures aim at, above the anchor cell's center.
    pub target: i64,
    /// Every cell of the footprint, anchor included, each with its hit-zone prism.
    pub cells: Vec<VolumeCell>,
}

/// One footprint cell of a [`FigureVolume`]: the space it stands on and the hit zone above it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeCell {
    /// The space the cell stands on, at its own surface level.
    pub space: Space,
    /// Bottom of the hit zone over this cell.
    pub bottom: i64,
    /// Top of the hit zone over this cell.
    pub top: i64,
}

impl FigureVolume {
    /// The columns the figure stands in, one per cell.
    pub fn hexes(&self) -> impl Iterator<Item = Hex> + '_ {
        self.cells.iter().map(|c| c.space.hex())
    }
}

/// What line of sight may ask about the board.
pub trait LosWorld {
    /// The terrain and feature blockers standing in the column at `hex`.
    fn blockers(&self, hex: Hex) -> &[Blocker];
    /// Figures on the board, in any order.
    fn figures(&self) -> Vec<FigureId>;
    /// The volume of a figure on the board, or `None` if it is off the board.
    fn volume(&self, figure: FigureId) -> Option<FigureVolume>;
}

/// The answer to one line-of-sight question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LosResult {
    /// Whether the policy's threshold of sample segments is clear.
    pub visible: bool,
    /// The share of sample segments that are clear.
    pub clear_fraction: Frac,
    /// The clear segments, for the client to draw.
    pub witnesses: Vec<Seg3>,
}

/// Whether one figure can see another, as the `los` slot defines it.
pub trait LineOfSight: Send + Sync {
    /// Casts from `from`'s target point to `to`'s volume.
    fn query(
        &self,
        world: &dyn LosWorld,
        from: FigureId,
        to: FigureId,
    ) -> Result<LosResult, LogicError>;
}
