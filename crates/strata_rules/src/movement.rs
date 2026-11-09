//! The movement service interface (design §7.4).
//!
//! A module in the `movement` slot provides [`Movement`]. It asks a [`MoveWorld`], which the host
//! implements over the board, the loaded content and the rule queries.

use strata_kernel::{DefId, Rotation, Space};
use strata_model::board::{CompiledBoard, Surface};
use strata_model::{FigureId, FootprintDef};

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
    /// The rotation of a figure on the board. Without an answer figures face the default way.
    fn rotation(&self, _figure: FigureId) -> Rotation {
        Rotation::default()
    }
    /// The footprint of a figure. Without one the figure covers its anchor space only.
    fn footprint(&self, _figure: FigureId) -> Option<&FootprintDef> {
        None
    }
    /// The figure covering `space` with any cell of its footprint, if any.
    fn occupant(&self, space: Space) -> Option<FigureId>;
    /// The spaces a figure covers where it stands, as the game state records them, in any
    /// order. Without an answer, movement places the footprint on its anchor itself.
    fn covered(&self, _figure: FigureId) -> Vec<Space> {
        Vec::new()
    }
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
    /// Whether a multi-cell figure covering `before` gives an enemy a reprisal by stepping to
    /// cover `after`, both in footprint offset order. The body is judged as a whole: an enemy is
    /// left only if some cell of `before` was engaged with it and no cell of `after` is.
    ///
    /// The default builds the answer from [`MoveWorld::provokes_reprisal`]: it says yes when some
    /// old cell would leave engagement whichever new cell it became. That never misses a
    /// reprisal, and is exact while one enemy is involved; with several it can flag a step that
    /// keeps each enemy through a different cell. A host that knows the engagement service
    /// should answer through it instead.
    fn body_provokes_reprisal(
        &self,
        figure: FigureId,
        before: &[Space],
        after: &[Space],
    ) -> Result<bool, LogicError> {
        for &from in before {
            let mut left_everywhere = true;
            for &to in after {
                if !self.provokes_reprisal(figure, from, to)? {
                    left_everywhere = false;
                    break;
                }
            }
            if left_everywhere {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// One space a figure can reach.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reach {
    /// The anchor space.
    pub space: Space,
    /// The figure's rotation there.
    pub rotation: Rotation,
    /// Every space the footprint covers there, in offset order; just `space` for one cell.
    pub cells: Vec<Space>,
    /// Total movement cost to get there.
    pub cost: i64,
    /// The anchor spaces walked, excluding the start and ending at `space`.
    pub path: Vec<Space>,
    /// The spaces the footprint covers where the figure starts and then after each step of
    /// `path`, in offset order, so one entry longer than `path`. A move hands this to
    /// [`Ctx::leave_engagement`](crate::Ctx::leave_engagement).
    pub walk: Vec<Vec<Space>>,
    /// Entering the space ends the move.
    pub stops_here: bool,
    /// The last step descended further than the fall limit.
    pub falls: bool,
    /// Some step of the path leaves engagement with an enemy who gets a reprisal.
    pub provokes_reprisal: bool,
    /// Another figure covers one of `cells`, so the figure can pass through but not stop.
    pub occupied: bool,
}

/// Where a figure can move to, as the `movement` slot defines it.
pub trait Movement: Send + Sync {
    /// Every (space, rotation) `figure` can reach, ordered by space and then rotation.
    fn reachable(&self, world: &dyn MoveWorld, figure: FigureId) -> Result<Vec<Reach>, LogicError>;

    /// The spaces `figure`'s footprint covers with its anchor on `anchor` turned by `rotation`,
    /// in offset order, with every cell standing where a move could stand it; `None` when some
    /// cell cannot. Placement uses this, so a figure is never put down where it could not move.
    /// Occupancy and `can_enter` are left to the caller.
    ///
    /// The default asks only for passable terrain: see [`passable_cells`].
    fn cells(
        &self,
        world: &dyn MoveWorld,
        figure: FigureId,
        anchor: Space,
        rotation: Rotation,
    ) -> Option<Vec<Space>> {
        passable_cells(world, figure, anchor, rotation)
    }
}

/// The spaces `figure`'s footprint covers with its anchor on `anchor` turned by `rotation`, in
/// offset order, with every cell on a surface that `fits` accepts (see
/// [`FootprintDef::spaces_where`]), or `None` when some cell has none. A figure without a
/// footprint covers its anchor alone, unchecked.
pub fn footprint_cells(
    world: &dyn MoveWorld,
    figure: FigureId,
    anchor: Space,
    rotation: Rotation,
    mut fits: impl FnMut(&Surface) -> bool,
) -> Option<Vec<Space>> {
    let Some(def) = world.footprint(figure) else {
        return Some(vec![anchor]);
    };
    let spaces = def.spaces_where(world.board(), anchor, rotation, |_, s| fits(s))?;
    Some(spaces.into_iter().map(|(space, _)| space).collect())
}

/// [`footprint_cells`] on passable terrain, whatever the headroom: where a figure is placed when
/// no movement module says more.
pub fn passable_cells(
    world: &dyn MoveWorld,
    figure: FigureId,
    anchor: Space,
    rotation: Rotation,
) -> Option<Vec<Space>> {
    footprint_cells(world, figure, anchor, rotation, |s| {
        world.terrain(s.terrain).passable
    })
}
