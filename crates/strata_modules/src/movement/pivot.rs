//! Turning in place: a two-cell figure pivots about its anchor at no cost (design §7.3).
//!
//! A pivot keeps the anchor where it is and turns the rotation one 60-degree step clockwise or
//! counterclockwise, so the rear cell swings to a neighbouring column. A wider turn is a chain of
//! pivots, each a search edge of its own, so the rear sweeps through every rotation in between
//! and each of those placements must be legal too: a wall or a figure in the way blocks the turn.
//! Pivots are free, in `snake` and `rigid` mode alike. A footprint of one cell has nothing to turn,
//! and one of three or more cells never pivots.

use strata_kernel::{Rotation, Space};
use strata_model::board::Surface;
use strata_rules::LogicError;

use super::body::{Body, stand};
use super::search::{Cache, Limits};

/// How many cells a footprint has when it may pivot.
pub(super) const PIVOT_CELLS: usize = 2;

/// The two turns a pivot may make: one step clockwise, then one step counterclockwise.
const TURNS: [i32; 2] = [1, -1];

/// One legal pivot.
pub(super) struct Pivot {
    /// The rotation the figure turns to.
    pub rotation: Rotation,
    /// The covered spaces after the turn, in offset order.
    pub cells: Vec<Space>,
    /// Some cell the turn newly covers stands on stop-on-enter terrain.
    pub stops_here: bool,
}

impl Body<'_> {
    /// The legal pivots of a figure whose anchor stands on `at.0`, turned by `at.1` and covering
    /// `before` in offset order.
    ///
    /// The anchor stays put; every cell of the new placement must stand where a step could stand
    /// it (a surface within the spread, passable, with headroom), no cell may be covered by
    /// another figure, and each cell the turn newly covers must pass `can_enter` and rise at most
    /// `max_climb` levels above the cell it was. Only a footprint of [`PIVOT_CELLS`] cells pivots.
    pub fn pivots(
        &self,
        cache: &mut Cache<'_>,
        limits: Limits,
        height: u32,
        at: (Space, Rotation),
        before: &[Space],
    ) -> Result<Vec<Pivot>, LogicError> {
        let def = match self {
            Body::Shaped(def) if def.cells.len() == PIVOT_CELLS => *def,
            _ => return Ok(Vec::new()),
        };
        let mut pivots = Vec::new();
        for turn in TURNS {
            let rotation = at.1.then(Rotation::from_steps(turn));
            let Some(spaces) = stand(def, cache, limits, height, at.0, rotation) else {
                continue;
            };
            if let Some(pivot) = check(cache, limits, rotation, spaces, before)? {
                pivots.push(pivot);
            }
        }
        Ok(pivots)
    }
}

/// The pivot to `rotation`, whose cells would stand on `spaces`, if the figure covering `before`
/// may make it.
fn check(
    cache: &mut Cache<'_>,
    limits: Limits,
    rotation: Rotation,
    spaces: Vec<(Space, Surface)>,
    before: &[Space],
) -> Result<Option<Pivot>, LogicError> {
    if spaces.len() != before.len() {
        return Ok(None);
    }
    let mut stops_here = false;
    for ((space, surface), was) in spaces.iter().zip(before) {
        let covered = cache.world.occupant(*space);
        if covered.is_some_and(|other| other != cache.figure) {
            return Ok(None);
        }
        if before.contains(space) {
            continue;
        }
        let climbs = space.z <= was.z.saturating_add(limits.max_climb);
        if !climbs || !cache.enters(*space)? {
            return Ok(None);
        }
        stops_here |= cache.terrain(surface.terrain).stops_here;
    }
    let cells = spaces.into_iter().map(|(space, _)| space).collect();
    Ok(Some(Pivot {
        rotation,
        cells,
        stops_here,
    }))
}
