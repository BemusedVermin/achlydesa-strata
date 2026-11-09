//! How a figure's footprint covers the board and moves over it (design §7.3).
//!
//! A one-cell figure is just its anchor. A larger footprint is placed with
//! [`FootprintDef::spaces_where`], each cell on the nearest surface it [`fits`] on, and its move
//! mode decides how it may turn: a `rigid` shape only
//! translates, while a `snake` shape turns so its rear follows the path its anchor, the leader,
//! has walked. Content loading allows `snake` only for two-cell footprints, since a fixed
//! rotation of three or more offsets cannot follow a bend.

use std::collections::BTreeSet;

use strata_kernel::{Hex, Rotation, Space};
use strata_model::board::Surface;
use strata_model::{FootprintDef, MoveMode};
use strata_rules::{LogicError, TerrainRules};

use super::search::{Cache, Headroom, Limits};

/// The figure's footprint, as the search uses it.
#[derive(Clone, Copy)]
pub(super) enum Body<'a> {
    /// One cell on the anchor: the anchor step's checks are the whole story.
    One,
    /// A footprint whose every cell must be placed and checked.
    Shaped(&'a FootprintDef),
}

/// Where a footprint ends up after a step, and what its cells stand on.
pub(super) struct Placed {
    /// The covered spaces, in offset order.
    pub cells: Vec<Space>,
    /// Some cell stands on stop-on-enter terrain.
    pub stops_here: bool,
}

/// Whether a figure `height` levels tall has room on `surface`.
fn roomy(limits: Limits, height: u32, surface: &Surface) -> bool {
    match (limits.headroom, surface.headroom) {
        (Headroom::FigureHeight, Some(free)) => free >= height,
        _ => true,
    }
}

/// Whether a figure `height` levels tall can stand on `surface`, whose terrain does `rules`: it
/// is passable and has room for the figure. Steps, footprint cells and placements all ask this.
pub(super) fn fits(rules: TerrainRules, limits: Limits, height: u32, surface: &Surface) -> bool {
    rules.passable && roomy(limits, height, surface)
}

impl<'a> Body<'a> {
    /// The body for `def`. A footprint of the anchor cell alone moves as one space.
    pub fn new(def: Option<&'a FootprintDef>) -> Self {
        match def {
            Some(def) if def.cells.cells() != [Hex::ORIGIN] => Body::Shaped(def),
            _ => Body::One,
        }
    }

    /// The rotations the figure may have once its anchor steps from `at` into `to`.
    ///
    /// A `snake` may take any rotation that covers the anchor's old hex and otherwise only the
    /// new anchor or cells the body already covered, so the rear walks where the leader walked
    /// rather than pivoting in place. Rotations that cover the same
    /// cells are kept once, preferring the one nearest the current rotation clockwise.
    pub fn turns(&self, at: (Space, Rotation), to: Hex) -> Vec<Rotation> {
        let def = match self {
            Body::Shaped(def) if def.move_mode == MoveMode::Snake => def,
            _ => return vec![at.1],
        };
        let mut walked: BTreeSet<Hex> = def.cells_at(at.0.hex(), at.1).into_iter().collect();
        walked.insert(to);
        let mut seen = BTreeSet::new();
        let mut turns = Vec::new();
        for k in 0..Rotation::COUNT {
            let rotation = at.1.then(Rotation::from_steps(i32::from(k)));
            let cells: BTreeSet<Hex> = def.cells_at(to, rotation).into_iter().collect();
            let trails = cells.contains(&at.0.hex());
            if trails && cells.is_subset(&walked) && seen.insert(cells) {
                turns.push(rotation);
            }
        }
        turns
    }

    /// Where the body stands at the start of a query, in offset order: the cells the world
    /// records for the figure, or else where its footprint would be placed, or else every cell
    /// at the anchor's level, so a search can start even where no legal placement is left.
    pub fn origin(
        &self,
        cache: &mut Cache<'_>,
        limits: Limits,
        height: u32,
        start: (Space, Rotation),
    ) -> Vec<Space> {
        let def = match self {
            Body::One => return vec![start.0],
            Body::Shaped(def) => def,
        };
        let hexes = def.cells_at(start.0.hex(), start.1);
        let held = cache.world.covered(cache.figure);
        let recorded = hexes.iter().map(|hex| {
            let mine = held.iter().find(|s| s.hex() == *hex);
            mine.copied()
        });
        if let Some(cells) = recorded.collect::<Option<Vec<Space>>>() {
            return cells;
        }
        if let Some(spaces) = stand(def, cache, limits, height, start.0, start.1) {
            return spaces.into_iter().map(|(space, _)| space).collect();
        }
        let level = hexes.into_iter().map(|hex| Space::at(hex, start.0.z));
        level.collect()
    }

    /// The placement with the anchor on `anchor`, or `None` when some cell cannot stand there:
    /// no surface, too wide a level spread, impassable terrain, no headroom or a `can_enter` veto.
    pub fn place(
        &self,
        cache: &mut Cache<'_>,
        limits: Limits,
        height: u32,
        anchor: Space,
        rotation: Rotation,
    ) -> Result<Option<Placed>, LogicError> {
        let def = match self {
            Body::One => {
                let cells = vec![anchor];
                let stops_here = false;
                return Ok(Some(Placed { cells, stops_here }));
            }
            Body::Shaped(def) => def,
        };
        let Some(spaces) = stand(def, cache, limits, height, anchor, rotation) else {
            return Ok(None);
        };
        let mut stops_here = false;
        for (space, surface) in &spaces {
            if !cache.enters(*space)? {
                return Ok(None);
            }
            stops_here |= cache.terrain(surface.terrain).stops_here;
        }
        let cells = spaces.into_iter().map(|(space, _)| space).collect();
        Ok(Some(Placed { cells, stops_here }))
    }

    /// Whether a step from covering `before` to covering `after`, both in offset order, keeps
    /// every cell within `max_climb`. Only a `rigid` shape's cells are checked one by one: the
    /// anchor's step is checked on its own, and it alone prices the move.
    pub fn climbs(&self, limits: Limits, before: &[Space], after: &[Space]) -> bool {
        match self {
            Body::Shaped(def) if def.move_mode == MoveMode::Rigid => {
                let mut cells = before.iter().zip(after);
                cells.all(|(from, to)| to.z <= from.z.saturating_add(limits.max_climb))
            }
            _ => true,
        }
    }
}

/// Where each cell of `def` stands with its anchor on `anchor`: on the nearest surface of its
/// column that the figure [`fits`] on (see [`FootprintDef::spaces_where`]).
fn stand(
    def: &FootprintDef,
    cache: &mut Cache<'_>,
    limits: Limits,
    height: u32,
    anchor: Space,
    rotation: Rotation,
) -> Option<Vec<(Space, Surface)>> {
    let world = cache.world;
    let fit =
        |_: Space, surface: &Surface| fits(cache.terrain(surface.terrain), limits, height, surface);
    def.spaces_where(world.board(), anchor, rotation, fit)
}
