//! The movement service interface (design §7.4).
//!
//! A module in the `movement` slot provides [`Movement`]. It asks a [`MoveWorld`], which the host
//! implements over the board, the loaded content and the rule queries.

use strata_kernel::{DefId, Space};
use strata_model::FigureId;
use strata_model::board::CompiledBoard;

use crate::query::LogicError;

/// What a terrain type does to movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainRules {
    /// Whether a figure can stand on it at all.
    pub passable: bool,
    /// Whether entering it ends the move.
    pub stops_here: bool,
}

/// What movement may ask about the board and the rules.
pub trait MoveWorld {
    /// The compiled board.
    fn board(&self) -> &CompiledBoard;
    /// The anchor space of a figure on the board.
    fn location(&self, figure: FigureId) -> Option<Space>;
    /// The figure standing on `space`, if any.
    fn occupant(&self, space: Space) -> Option<FigureId>;
    /// How many levels of headroom the figure needs.
    fn height(&self, figure: FigureId) -> u32;
    /// The movement allowance the figure starts a move with.
    fn allowance(&self, figure: FigureId) -> Result<i64, LogicError>;
    /// The tags of a terrain type, as movement sees them.
    fn terrain(&self, terrain: DefId) -> TerrainRules;
    /// The `move_cost` query for entering `terrain` across a level change of `dz`.
    fn move_cost(&self, figure: FigureId, terrain: DefId, dz: i32) -> Result<i64, LogicError>;
    /// The `can_climb` query for entering `terrain` across a level rise of `dz`.
    fn can_climb(&self, figure: FigureId, terrain: DefId, dz: i32) -> Result<bool, LogicError>;
    /// The `can_enter` query for entering `to`.
    fn can_enter(&self, figure: FigureId, to: Space) -> Result<bool, LogicError>;
    /// Whether stepping from `from` to `to` gives an enemy a reprisal, as the `engagement` slot
    /// decides. Without an engagement module no step does.
    fn provokes_reprisal(
        &self,
        _figure: FigureId,
        _from: Space,
        _to: Space,
    ) -> Result<bool, LogicError> {
        Ok(false)
    }
}

/// One space a figure can reach.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reach {
    /// The space.
    pub space: Space,
    /// Total movement cost to get there.
    pub cost: i64,
    /// The spaces walked, excluding the start and ending at `space`.
    pub path: Vec<Space>,
    /// Entering the space ends the move.
    pub stops_here: bool,
    /// The last step descended further than the fall limit.
    pub falls: bool,
    /// Some step of the path leaves engagement with an enemy who gets a reprisal.
    pub provokes_reprisal: bool,
    /// Another figure stands here, so the figure can pass through but not stop.
    pub occupied: bool,
}

/// Where a figure can move to, as the `movement` slot defines it.
pub trait Movement: Send + Sync {
    /// Every space `figure` can reach, ordered by [`Space`].
    fn reachable(&self, world: &dyn MoveWorld, figure: FigureId) -> Result<Vec<Reach>, LogicError>;
}
